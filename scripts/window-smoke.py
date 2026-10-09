"""Windows geometry regression: run against an isolated profile with CDP on 9222.

Requires an existing paused track. Does not change the library or start playback.
Checks native window/client/WebView bounds as well as the DOM: a browser screenshot
alone cannot detect the exposed native frame from the maximized-fullscreen bug.
"""
import ctypes as c
from ctypes import wintypes as w
import json
from pathlib import Path
from playwright.sync_api import sync_playwright

user32 = c.WinDLL('user32', use_last_error=True)
user32.SetProcessDpiAwarenessContext(c.c_void_p(-4))
callback = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
user32.EnumWindows.argtypes = [callback, w.LPARAM]
user32.EnumChildWindows.argtypes = [w.HWND, callback, w.LPARAM]
user32.GetClassNameW.argtypes = [w.HWND, w.LPWSTR, c.c_int]
user32.GetWindowRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
user32.GetClientRect.argtypes = [w.HWND, c.POINTER(w.RECT)]
user32.ClientToScreen.argtypes = [w.HWND, c.POINTER(w.POINT)]
user32.IsZoomed.argtypes = [w.HWND]
user32.IsWindowVisible.argtypes = [w.HWND]
user32.MonitorFromWindow.argtypes = [w.HWND, w.DWORD]
user32.MonitorFromWindow.restype = w.HANDLE


class MonitorInfo(c.Structure):
    _fields_ = [('cbSize', w.DWORD), ('monitor', w.RECT), ('work', w.RECT), ('flags', w.DWORD)]


user32.GetMonitorInfoW.argtypes = [w.HANDLE, c.POINTER(MonitorInfo)]


def rect(value):
    return [value.left, value.top, value.right, value.bottom]


def find_window(parent, name):
    found = []
    def visit(handle, _):
        label = c.create_unicode_buffer(256)
        user32.GetClassNameW(handle, label, 256)
        if label.value == name and user32.IsWindowVisible(handle):
            found.append(handle)
        return True
    if parent:
        user32.EnumChildWindows(parent, callback(visit), 0)
    else:
        user32.EnumWindows(callback(visit), 0)
    assert len(found) == 1, (name, found)
    return found[0]


def bounds(handle):
    outer, client, origin = w.RECT(), w.RECT(), w.POINT()
    user32.GetWindowRect(handle, c.byref(outer))
    user32.GetClientRect(handle, c.byref(client))
    user32.ClientToScreen(handle, c.byref(origin))
    return dict(outer=rect(outer), size=[client.right, client.bottom],
                origin=[origin.x, origin.y], maximized=bool(user32.IsZoomed(handle)))


with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9222')
    page = browser.contexts[0].pages[0]
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))
    handle = find_window(None, 'Tauri Window')
    webview = find_window(handle, 'WRY_WEBVIEW')
    monitor = MonitorInfo(cbSize=c.sizeof(MonitorInfo))
    user32.GetMonitorInfoW(user32.MonitorFromWindow(handle, 2), c.byref(monitor))
    screen = rect(monitor.monitor)
    screen_size = [screen[2] - screen[0], screen[3] - screen[1]]
    report = []

    def invoke(action):
        return page.evaluate('(action)=>window.__TAURI_INTERNALS__.invoke("window_action",{action})', action)

    def check(label, fullscreen=False, expected=None, mini=False):
        page.wait_for_timeout(140)
        window, content = bounds(handle), bounds(webview)
        dom = page.evaluate('({size:[innerWidth,innerHeight],dpr:devicePixelRatio,overflow:document.documentElement.scrollWidth>innerWidth})')
        assert content['size'] == window['size'], (label, window, content)
        assert content['origin'] == window['origin'], (label, window, content)
        assert [round(v * dom['dpr']) for v in dom['size']] == window['size'], (label, dom, window)
        assert not dom['overflow'], label
        if fullscreen:
            assert window['outer'] == screen and window['origin'] == screen[:2], (label, window, screen)
            assert window['size'] == screen_size and not window['maximized'], (label, window)
            assert page.locator('.player.immersive').count() == 1
        if mini:
            assert dom['size'] == [430, 260], (label, dom)
        if expected:
            assert window == expected, (label, window, expected)
        report.append(dict(label=label, **window))
        return window

    page.keyboard.press('Escape')
    if bounds(handle)['maximized']:
        invoke('maximize')
    normal = check('normal')
    for cycle in range(3):
        for maximized in [False, True]:
            if maximized:
                invoke('maximize')
            original = check(f'{cycle}: start maximized={maximized}')
            # A burst of shortcuts must not interleave native mode transitions.
            page.evaluate('''()=>{for(let i=0;i<8;i++)document.body.dispatchEvent(new KeyboardEvent('keydown',{key:'f',bubbles:true}));}''')
            check('fullscreen', fullscreen=True)
            page.keyboard.press('Escape')
            check('restore fullscreen', expected=original)
            page.keyboard.press('m')
            check('mini', mini=True)
            page.keyboard.press('f')
            check('mini to fullscreen', fullscreen=True)
            page.keyboard.press('m')
            check('fullscreen to mini', mini=True)
            page.keyboard.press('Escape')
            check('restore mini chain', expected=original)
            if maximized:
                invoke('maximize')
            check('original normal bounds', expected=normal)
    assert not errors, errors
    Path('.tools/window-regression-report.json').write_text(json.dumps(report, indent=2))
    print(json.dumps(dict(checks=len(report), fullscreen_size=screen_size, restored_normal=normal,
                          page_errors=errors), indent=2))
    browser.close()
