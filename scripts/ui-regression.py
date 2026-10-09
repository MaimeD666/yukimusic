"""Player appearance, seek feedback, artist artwork and shared hover behavior.
Run only against an isolated native profile with CDP enabled.
"""
from playwright.sync_api import sync_playwright
from pathlib import Path
import json,time
report={}
with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222');page=browser.contexts[0].pages[0]
    errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
    def invoke(command,**args):return page.evaluate('(v)=>window.__TAURI_INTERNALS__.invoke(v.command,v.args)',{'command':command,'args':args})
    def action(name,value=None):invoke('player_action',action=name,value=value)
    library=invoke('get_library');track=library['tracks'][0]
    expected=invoke('artist_profile',id=track['artistId'])
    legacy={**expected,'artwork':track['artwork'],'artworkVerified':False}
    invoke('save_library',library={**library,'artists':[legacy]})
    page.reload();page.wait_for_timeout(1800)
    stored=invoke('get_library')['artists'][0]
    assert stored['artworkVerified'] and stored['artwork']==expected['artwork']
    report['artist_uses_profile_artwork']=True
    for entry in invoke('snapshot')['queue']:action('remove',entry['key'])
    page.wait_for_timeout(500)
    assert page.locator('.player').count()==0
    action('volume',0);action('play',{'tracks':library['tracks'],'index':0})
    page.wait_for_timeout(1500);assert page.locator('.player').count()==1
    state=invoke('snapshot')
    if state['playing']:action('toggle')
    page.wait_for_timeout(700);assert page.locator('.player').count()==1
    report['empty_hidden_and_pause_visible']=True
    sequence=page.evaluate('''async()=>{
      const slider=document.querySelector('.timeline input');slider.value='30';
      slider.dispatchEvent(new Event('input',{bubbles:true}));slider.dispatchEvent(new Event('change',{bubbles:true}));
      const values=[];const start=performance.now();while(performance.now()-start<450){await new Promise(requestAnimationFrame);values.push(Number(slider.value));}return values;
    }''')
    assert all(abs(value-30)<.15 for value in sequence),sequence
    report['seek_thumb_stable']=True
    page.locator('.search input').fill('meat computer');page.wait_for_timeout(1600)
    page.locator('.artist-item').first.hover()
    assert page.locator('.artist-portrait').first.evaluate('e=>getComputedStyle(e).transform')=='none'
    icon_offset=page.locator('.artist-save').first.evaluate('''e=>{const b=e.getBoundingClientRect(),s=e.querySelector('svg').getBoundingClientRect();return Math.abs(b.x+b.width/2-s.x-s.width/2)+Math.abs(b.y+b.height/2-s.y-s.height/2)}''')
    assert icon_offset<1
    page.locator('.playlist-open').first.hover()
    assert page.locator('.playlist-image img').first.evaluate('e=>getComputedStyle(e).transform')=='none'
    report['hover_without_motion']=True
    report['artist_icon_center_error_px']=icon_offset
    report['page_errors']=errors;assert not errors,errors
    Path('.tools/ui-regression-report.json').write_text(json.dumps(report,indent=2),encoding='utf8');print(json.dumps(report,indent=2))
    browser.close()
