"""Native latency and queue regression checks. Use an isolated profile; sound is muted."""
from pathlib import Path
import json,time
from playwright.sync_api import sync_playwright

report={}
with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222')
    page=browser.contexts[0].pages[0]
    def invoke(command,**args):
        return page.evaluate('(v)=>window.__TAURI_INTERNALS__.invoke(v.command,v.args)',{'command':command,'args':args})
    def action(name,value=None):invoke('player_action',action=name,value=value)
    def wait(predicate,timeout=20):
        until=time.monotonic()+timeout
        while time.monotonic()<until:
            state=invoke('snapshot')
            if predicate(state):return state
            page.wait_for_timeout(25)
        raise AssertionError(state.get('error') or 'Playback timeout')
    state=json.loads(Path('.tools/before-speed-test.json').read_text(encoding='utf8'))
    tracks=[e['track'] for e in state['queue'] if e['track']['available']][:4]
    action('volume',0);action('repeat','off')
    if invoke('snapshot')['shuffle']:action('shuffle')
    start=time.perf_counter();action('play',{'tracks':tracks,'index':0});wait(lambda s:s['playing'])
    report['cold_start_ms']=round((time.perf_counter()-start)*1000)
    page.wait_for_timeout(2500)
    report['seek_ms']=[]
    for target in [30,8,45,16]:
        start=time.perf_counter();action('seek',target)
        wait(lambda s:s['playing'] and abs(s['position']-target)<1)
        report['seek_ms'].append(round((time.perf_counter()-start)*1000))
        page.wait_for_timeout(450)
    start=time.perf_counter();action('next');wait(lambda s:s['playing'] and next(e['track']['id'] for e in s['queue'] if e['key']==s['currentKey'])==tracks[1]['id'])
    report['prepared_next_ms']=round((time.perf_counter()-start)*1000)
    action('toggle');wait(lambda s:not s['playing']);action('seek',20);page.wait_for_timeout(400)
    assert not invoke('snapshot')['playing']
    action('seek',tracks[1]['duration']);page.wait_for_timeout(200)
    assert invoke('snapshot')['error'] is None
    action('toggle');wait(lambda s:s['playing'] and s['position']<2)
    fake={**tracks[0],'id':999999999999999,'title':'Unavailable test'}
    action('repeat','one');action('play',{'tracks':[fake,fake,tracks[0]],'index':0})
    s=wait(lambda s:s['playing'] and next(e['track']['id'] for e in s['queue'] if e['key']==s['currentKey'])==tracks[0]['id'])
    assert not s['queue'][0]['track']['available'] and not s['queue'][1]['track']['available']
    report['unavailable_duplicates_skipped']=True
    Path('.tools/latency-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print(json.dumps(report),flush=True)
    action('toggle');action('repeat','off')
    page.locator('.search input').fill('')
    page.wait_for_timeout(100)
    page.locator('.search input').fill('meat computer')
    for _ in range(120):
        if page.locator('.track-table').count() and int(page.locator('.track-table').get_attribute('aria-rowcount'))>=30:break
        page.wait_for_timeout(100)
    initial=int(page.locator('.track-table').get_attribute('aria-rowcount'))
    page.locator('.track-scroll').evaluate('e=>e.scrollTop=e.scrollHeight')
    for _ in range(120):
        if int(page.locator('.track-table').get_attribute('aria-rowcount'))>initial:break
        page.wait_for_timeout(100)
    count=int(page.locator('.track-table').get_attribute('aria-rowcount'))
    assert count>initial
    page.wait_for_timeout(1300)
    assert int(page.locator('.track-table').get_attribute('aria-rowcount'))==count
    assert page.locator('.track-row').count()<30
    report['autoload']={'initial':initial,'after_scroll':count,'rendered_rows':page.locator('.track-row').count()}
    Path('.tools/latency-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print(json.dumps(report,indent=2),flush=True)
    browser.close()
