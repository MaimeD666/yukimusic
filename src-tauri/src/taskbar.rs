//! Windows thumbnail transport, owned by the main HWND rather than the WebView.
//! The audio worker posts only state changes; COM and icon ownership stay on the UI thread.
use crate::{audio::Message, models::Playback};
use std::sync::{
    atomic::{AtomicIsize, AtomicU8, Ordering},
    mpsc::Sender,
};
use windows::{
    core::w,
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::{
            Com::{CoCreateInstance, CLSCTX_INPROC_SERVER},
            Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
        },
        UI::{
            Shell::{
                DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass,
                TaskbarList, THBF_DISABLED, THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON,
                THB_TOOLTIP, THUMBBUTTON,
            },
            WindowsAndMessaging::{
                CreateIconFromResourceEx, DestroyIcon, PostMessageW, RegisterWindowMessageW, HICON,
                LR_DEFAULTCOLOR, WM_APP, WM_COMMAND, WM_NCDESTROY, WM_SETTINGCHANGE,
                WM_THEMECHANGED,
            },
        },
    },
};

const UPDATE: u32 = WM_APP + 0x341;
const SUBCLASS: usize = 0x59554b49;
const PREVIOUS: u32 = 0x5101;
const TOGGLE: u32 = 0x5102;
const NEXT: u32 = 0x5103;
const ENABLED: u8 = 1;
const PLAYING: u8 = 2;
static WINDOW: AtomicIsize = AtomicIsize::new(0);
static STATUS: AtomicU8 = AtomicU8::new(0);

pub fn update(state: &Playback) {
    let status = u8::from(state.track().is_some()) * ENABLED
        | u8::from(state.playing || state.loading) * PLAYING;
    if STATUS.swap(status, Ordering::AcqRel) != status {
        let handle = WINDOW.load(Ordering::Acquire);
        if handle != 0 {
            unsafe {
                let _ = PostMessageW(Some(HWND(handle as _)), UPDATE, WPARAM(0), LPARAM(0));
            }
        }
    }
}

struct Icons([HICON; 4]);
impl Icons {
    fn new() -> windows::core::Result<Self> {
        let mut light = 0u32;
        let mut size = 4u32;
        unsafe {
            let _ = RegGetValueW(
                HKEY_CURRENT_USER,
                w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
                w!("SystemUsesLightTheme"),
                RRF_RT_REG_DWORD,
                None,
                Some((&mut light as *mut u32).cast()),
                Some(&mut size),
            );
        }
        let assets: [&[u8]; 4] = if light != 0 {
            [
                include_bytes!("../icons/transport/previous-light.ico"),
                include_bytes!("../icons/transport/play-light.ico"),
                include_bytes!("../icons/transport/pause-light.ico"),
                include_bytes!("../icons/transport/next-light.ico"),
            ]
        } else {
            [
                include_bytes!("../icons/transport/previous-dark.ico"),
                include_bytes!("../icons/transport/play-dark.ico"),
                include_bytes!("../icons/transport/pause-dark.ico"),
                include_bytes!("../icons/transport/next-dark.ico"),
            ]
        };
        let mut icons = Self([HICON::default(); 4]);
        for (icon, data) in icons.0.iter_mut().zip(assets) {
            // Embedded ICO files contain exactly one 32 px PNG image.
            let offset = u32::from_le_bytes(data[18..22].try_into().unwrap()) as usize;
            *icon = unsafe {
                CreateIconFromResourceEx(
                    &data[offset..],
                    true,
                    0x0003_0000,
                    32,
                    32,
                    LR_DEFAULTCOLOR,
                )?
            };
        }
        Ok(icons)
    }
}
impl Drop for Icons {
    fn drop(&mut self) {
        for icon in self.0 {
            if !icon.is_invalid() {
                unsafe {
                    let _ = DestroyIcon(icon);
                }
            }
        }
    }
}

struct Toolbar {
    sender: Sender<Message>,
    ready_message: u32,
    taskbar: Option<ITaskbarList3>,
    icons: Icons,
}
impl Toolbar {
    fn buttons(&self) -> [THUMBBUTTON; 3] {
        let status = STATUS.load(Ordering::Acquire);
        let playing = status & PLAYING != 0;
        let button = |id, icon, text: &str| {
            let mut b = THUMBBUTTON {
                dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                iId: id,
                hIcon: self.icons.0[icon],
                dwFlags: if status & ENABLED != 0 {
                    THBF_ENABLED
                } else {
                    THBF_DISABLED
                },
                ..Default::default()
            };
            for (slot, value) in b.szTip.iter_mut().take(259).zip(text.encode_utf16()) {
                *slot = value;
            }
            b
        };
        [
            button(PREVIOUS, 0, "Предыдущий трек"),
            button(
                TOGGLE,
                if playing { 2 } else { 1 },
                if playing {
                    "Пауза"
                } else {
                    "Воспроизвести"
                },
            ),
            button(NEXT, 3, "Следующий трек"),
        ]
    }
    unsafe fn initialize(&mut self, hwnd: HWND) -> windows::core::Result<()> {
        // Recreated after Explorer restarts too. Never call before TaskbarButtonCreated.
        self.taskbar = None;
        let taskbar: ITaskbarList3 = CoCreateInstance(&TaskbarList, None, CLSCTX_INPROC_SERVER)?;
        taskbar.HrInit()?;
        taskbar.ThumbBarAddButtons(hwnd, &self.buttons())?;
        self.taskbar = Some(taskbar);
        #[cfg(debug_assertions)]
        eprintln!("Thumbnail toolbar ready");
        Ok(())
    }
    unsafe fn refresh(&self, hwnd: HWND) {
        if let Some(taskbar) = &self.taskbar {
            let _ = taskbar.ThumbBarUpdateButtons(hwnd, &self.buttons());
        }
    }
}

unsafe extern "system" fn subclass(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    data: usize,
) -> LRESULT {
    let toolbar = &mut *(data as *mut Toolbar);
    if message == toolbar.ready_message {
        if let Err(error) = toolbar.initialize(hwnd) {
            eprintln!("Thumbnail toolbar: {error}");
        }
    } else if message == UPDATE {
        toolbar.refresh(hwnd);
        return LRESULT(0);
    } else if message == WM_SETTINGCHANGE || message == WM_THEMECHANGED {
        if let Ok(icons) = Icons::new() {
            toolbar.icons = icons;
            toolbar.refresh(hwnd);
        }
    } else if message == WM_COMMAND && ((wparam.0 >> 16) & 0xffff) == THBN_CLICKED as usize {
        let action = match (wparam.0 & 0xffff) as u32 {
            PREVIOUS => Some("previous"),
            TOGGLE => Some("toggle"),
            NEXT => Some("next"),
            _ => None,
        };
        if let Some(action) = action {
            if STATUS.load(Ordering::Acquire) & ENABLED != 0 {
                let _ = toolbar
                    .sender
                    .send(Message::Action(action.into(), serde_json::Value::Null));
            }
            return LRESULT(0);
        }
    } else if message == WM_NCDESTROY {
        let _ = WINDOW.compare_exchange(hwnd.0 as isize, 0, Ordering::AcqRel, Ordering::Acquire);
        let _ = RemoveWindowSubclass(hwnd, Some(subclass), SUBCLASS);
        drop(Box::from_raw(data as *mut Toolbar));
    }
    DefSubclassProc(hwnd, message, wparam, lparam)
}

pub fn attach(window: &tauri::WebviewWindow, sender: Sender<Message>) {
    let target = window.clone();
    let _ = window.run_on_main_thread(move || {
        let Ok(handle) = target.hwnd() else { return };
        let Ok(icons) = Icons::new() else { return };
        let hwnd = HWND(handle.0);
        let toolbar = Box::new(Toolbar {
            sender,
            icons,
            taskbar: None,
            ready_message: unsafe { RegisterWindowMessageW(w!("TaskbarButtonCreated")) },
        });
        let data = Box::into_raw(toolbar);
        // show_main creates the window hidden, installs this hook, then shows it.
        // WM_NCDESTROY owns cleanup, including when the WebView is released in the tray.
        unsafe {
            if SetWindowSubclass(hwnd, Some(subclass), SUBCLASS, data as usize).as_bool() {
                WINDOW.store(hwnd.0 as isize, Ordering::Release);
            } else {
                drop(Box::from_raw(data));
            }
        }
    });
}
