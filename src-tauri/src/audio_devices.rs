//! MMDevice notifications wake the audio engine only when endpoints change.
use crate::audio::Message;
use std::{cell::RefCell, sync::mpsc::Sender};
use windows::{
    core::{implement, PCWSTR},
    Win32::{
        Foundation::PROPERTYKEY,
        Media::Audio::{
            eConsole, eRender, EDataFlow, ERole, IMMDeviceEnumerator, IMMNotificationClient,
            IMMNotificationClient_Impl, MMDeviceEnumerator, DEVICE_STATE,
        },
        System::Com::{
            CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_INPROC_SERVER,
            COINIT_APARTMENTTHREADED,
        },
    },
};

#[implement(IMMNotificationClient)]
struct Notifications {
    sender: Sender<Message>,
}
impl IMMNotificationClient_Impl for Notifications_Impl {
    fn OnDefaultDeviceChanged(
        &self,
        flow: EDataFlow,
        role: ERole,
        _: &PCWSTR,
    ) -> windows::core::Result<()> {
        // CPAL/rodio opens the eConsole rendering endpoint on Windows.
        if flow == eRender && role == eConsole {
            let _ = self.sender.send(Message::OutputChanged);
        }
        Ok(())
    }
    fn OnDeviceStateChanged(&self, _: &PCWSTR, _: DEVICE_STATE) -> windows::core::Result<()> {
        let _ = self.sender.send(Message::OutputChanged);
        Ok(())
    }
    fn OnDeviceAdded(&self, _: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.sender.send(Message::OutputChanged);
        Ok(())
    }
    fn OnDeviceRemoved(&self, _: &PCWSTR) -> windows::core::Result<()> {
        let _ = self.sender.send(Message::OutputChanged);
        Ok(())
    }
    fn OnPropertyValueChanged(&self, _: &PCWSTR, _: &PROPERTYKEY) -> windows::core::Result<()> {
        Ok(())
    }
}
struct Watcher {
    enumerator: IMMDeviceEnumerator,
    callback: IMMNotificationClient,
    _apartment: Apartment,
}
struct Apartment(bool);
impl Drop for Apartment {
    fn drop(&mut self) {
        if self.0 {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
impl Drop for Watcher {
    fn drop(&mut self) {
        unsafe {
            let _ = self
                .enumerator
                .UnregisterEndpointNotificationCallback(&self.callback);
        }
    }
}
thread_local! { static WATCHER: RefCell<Option<Watcher>> = const { RefCell::new(None) }; }

pub fn attach(sender: Sender<Message>) -> windows::core::Result<()> {
    unsafe {
        let apartment = Apartment(CoInitializeEx(None, COINIT_APARTMENTTHREADED).is_ok());
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER)?;
        let callback: IMMNotificationClient = Notifications {
            sender: sender.clone(),
        }
        .into();
        enumerator.RegisterEndpointNotificationCallback(&callback)?;
        WATCHER.with(|slot| {
            *slot.borrow_mut() = Some(Watcher {
                enumerator,
                callback,
                _apartment: apartment,
            })
        });
        // Cover a default-device change during audio engine initialization.
        let _ = sender.send(Message::OutputChanged);
        #[cfg(debug_assertions)]
        eprintln!("Audio device notifications ready");
        Ok(())
    }
}
pub fn detach() {
    WATCHER.with(|slot| {
        slot.borrow_mut().take();
    });
}

pub fn default_id() -> Option<String> {
    unsafe {
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_INPROC_SERVER).ok()?;
        let device = enumerator.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let id = device.GetId().ok()?;
        let text = id.to_string().ok();
        CoTaskMemFree(Some(id.0.cast()));
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::Win32::Media::Audio::{eCapture, eCommunications};
    #[test]
    fn only_default_playback_role_wakes_engine() {
        let (sender, receiver) = std::sync::mpsc::channel();
        let callback: IMMNotificationClient = Notifications { sender }.into();
        unsafe {
            callback
                .OnDefaultDeviceChanged(eCapture, eConsole, PCWSTR::null())
                .unwrap();
            callback
                .OnDefaultDeviceChanged(eRender, eCommunications, PCWSTR::null())
                .unwrap();
            assert!(receiver.try_recv().is_err());
            callback
                .OnDefaultDeviceChanged(eRender, eConsole, PCWSTR::null())
                .unwrap();
            assert!(matches!(receiver.try_recv(), Ok(Message::OutputChanged)));
        }
    }
}
