"""Measure the complete application process tree, including WebView2 and FFmpeg."""
import json, os, subprocess, sys, time
from pathlib import Path
sys.path.insert(0, str(Path('.tools/python').resolve()))
import psutil
from playwright.sync_api import sync_playwright

root = next(p for p in psutil.process_iter(['name','exe']) if p.info['name']=='yukicloud.exe' and '\\target\\release\\' in (p.info['exe'] or ''))
def measure(seconds=10):
    processes=[root]+root.children(recursive=True)
    def cpu(p):
        try: t=p.cpu_times(); return t.user+t.system
        except psutil.Error: return 0
    initial={p.pid:cpu(p) for p in processes}
    start=time.monotonic();time.sleep(seconds);elapsed=time.monotonic()-start
    total=0;rss=0;private=0;names=[]
    for p in [root]+root.children(recursive=True):
        try:
            total+=max(0,cpu(p)-initial.get(p.pid,0));m=p.memory_info();rss+=m.rss;private+=m.private;names.append(p.name())
        except psutil.Error: pass
    return {'cpu_percent_all_cores':round(total/elapsed/psutil.cpu_count()*100,3),'working_set_mib':round(rss/1048576,1),'private_commit_mib':round(private/1048576,1),'processes':names,'sample_seconds':round(elapsed,1)}

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222');page=browser.contexts[0].pages[0]
    def invoke(command,**args):return page.evaluate('(v)=>window.__TAURI_INTERNALS__.invoke(v.command,v.args)',{'command':command,'args':args})
    invoke('player_action',action='volume',value=0)
    state=invoke('snapshot')
    if not state['playing']:invoke('player_action',action='toggle',value=None)
    time.sleep(5)
    report={'visible_playing':measure()}
    page.get_by_role('button',name='Полноэкранный плеер',exact=True).click()
    time.sleep(1)
    bass=page.get_by_role('button',name='Бас-режим',exact=True)
    if bass.get_attribute('aria-pressed')!='true':bass.click()
    time.sleep(2)
    report['fullscreen_bass']=measure()
    page.keyboard.press('Escape')
    time.sleep(1)
    before=invoke('snapshot')['position']
    invoke('window_action',action='close')
    time.sleep(33)
    report['tray_playing']=measure()
    report['webview_unloaded']=not any('msedgewebview2' in name for name in report['tray_playing']['processes'])
    subprocess.Popen([root.exe()],env=os.environ.copy(),creationflags=0x08000000).wait(timeout=10)
    time.sleep(3)
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222');page=browser.contexts[0].pages[0]
    assert page.evaluate('!!window.__TAURI_INTERNALS__')
    state=invoke('snapshot')
    report['restored']={'playing':state['playing'],'queue_length':len(state['queue']),'position':state['position'],'before_hide_position':before}
    assert state['playing'] and state['queue'],state.get('error')
    invoke('player_action',action='toggle',value=None)
    time.sleep(1)
    invoke('window_action',action='close')
    time.sleep(33)
    report['tray_paused']=measure()
    Path('.tools/performance-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print(json.dumps(report,indent=2),flush=True)
    subprocess.Popen([root.exe()],env=os.environ.copy(),creationflags=0x08000000).wait(timeout=10)
    time.sleep(3)
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222');page=browser.contexts[0].pages[0]
    invoke('window_action',action='exit')
