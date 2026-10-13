"""Real SoundCloud/native smoke test. Run against an isolated YUKICLOUD_DATA_DIR.
Requires Python Playwright and WebView2 CDP on 127.0.0.1:9222. Audio stays muted.
"""
import json
import time
from pathlib import Path
from playwright.sync_api import sync_playwright

report = {}
shots = Path('.tools/screenshots')
shots.mkdir(exist_ok=True)
with sync_playwright() as p:
    browser = p.chromium.connect_over_cdp('http://127.0.0.1:9222')
    page = browser.contexts[0].pages[0]
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))
    def invoke(command, **args):
        return page.evaluate('(v)=>window.__TAURI_INTERNALS__.invoke(v.command,v.args)', {'command':command,'args':args})
    def action(name, value=None):
        invoke('player_action', action=name, value=value)
        page.wait_for_timeout(180)
    def wait_state(predicate, seconds=30):
        deadline=time.monotonic()+seconds
        while time.monotonic()<deadline:
            state=invoke('snapshot')
            if predicate(state): return state
            page.wait_for_timeout(250)
        raise AssertionError(f'Playback timed out: {state.get("error")}, {state.get("position")}')

    page.locator('.search input').fill('meat computer')
    page.wait_for_function('document.querySelectorAll(".track-row").length > 0')
    found=invoke('search', query='meat computer')
    assert all(found[k] for k in ['artists','playlists','tracks']), found['errors']
    report['search']={k:len(found[k]) for k in ['artists','playlists','tracks']}
    playlist=invoke('playlist_tracks', id=found['playlists'][0]['id'])
    assert len(playlist['tracks'])==playlist['total'] and len(playlist['tracks'])>5
    report['playlist']={'loaded':len(playlist['tracks']),'total':playlist['total']}
    next_url=None; seen=set(); artist_tracks=[]; pages=0
    while True:
        catalog=invoke('artist_tracks', id=found['artists'][0]['id'], next=next_url)
        artist_tracks.extend(catalog['tracks']); pages+=1
        next_url=catalog['next']
        if not next_url: break
        assert next_url not in seen
        seen.add(next_url)
    report['artist']={'tracks':len(artist_tracks),'pages':pages}
    assert pages>1 and len(artist_tracks)>100
    tracks=[t for t in found['tracks'] if t['available']][:4]
    action('volume',0)
    action('play',{'tracks':tracks,'index':0})
    state=wait_state(lambda s:s['playing'] and s['position']>2)
    report['playback_started']=state['position']
    page.wait_for_timeout(700)
    page.screenshot(path=str(shots/'native-playing.png'))
    action('eq',{'bands':[3,2,0,-1,-2,0,1,2,1,0],'enabled':True})
    action('seek',25)
    state=wait_state(lambda s:s['playing'] and s['position']>=26)
    report['seek']=state['position']
    action('enqueue',{'tracks':[tracks[0],tracks[0]],'next':True})
    state=invoke('snapshot')
    keys=[q['key'] for q in state['queue']]
    assert len(keys)==len(set(keys))==6
    current=state['currentKey']
    action('move',{'from':keys[-1],'to':keys[1]})
    assert invoke('snapshot')['currentKey']==current
    action('shuffle'); action('shuffle')
    assert invoke('snapshot')['currentKey']==current
    action('repeat','one'); action('next')
    state=wait_state(lambda s:s['playing'] and s['currentKey']!=current)
    report['queue']='duplicate occurrences, move, shuffle, manual next under repeat-one passed'
    page.get_by_role('button',name='Открыть очередь',exact=True).click()
    page.wait_for_timeout(400);page.screenshot(path=str(shots/'queue.png'))
    page.keyboard.press('Escape')
    page.get_by_role('button',name='Эквалайзер',exact=True).click()
    page.wait_for_timeout(400);page.screenshot(path=str(shots/'equalizer.png'))
    page.keyboard.press('Escape')
    page.get_by_role('button',name='Полноэкранный плеер',exact=True).click()
    page.wait_for_timeout(900);page.screenshot(path=str(shots/'fullscreen.png'))
    page.keyboard.press('Escape')
    page.get_by_role('button',name='Мини-плеер',exact=True).click()
    page.wait_for_timeout(700);page.screenshot(path=str(shots/'mini.png'))
    report['mini_size']=page.evaluate('({width:innerWidth,height:innerHeight})')
    assert report['mini_size']=={'width':430,'height':260}
    page.keyboard.press('Escape')
    action('toggle')
    a=invoke('snapshot')['position'];page.wait_for_timeout(1200);b=invoke('snapshot')['position']
    assert abs(a-b)<0.5, (a,b)
    report['pause']='position stable'
    invoke('visualization',visible=True,enabled=False)
    action('volume',0.01)
    assert invoke('snapshot')['volume']>0
    action('volume',0)
    library={'artists':found['artists'][:2],'playlists':found['playlists'][:1],'tracks':tracks}
    invoke('save_library',library=library)
    assert invoke('get_library')==library
    report['library']='saved and read back'
    report['page_errors']=errors
    assert not errors
    print(json.dumps(report,ensure_ascii=False,indent=2),flush=True)
    Path('.tools/smoke-report.json').write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding='utf8')
    browser.close()
