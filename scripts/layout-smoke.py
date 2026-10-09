"""Verify the desktop layouts and playlist actions against the native application."""
from playwright.sync_api import sync_playwright
from pathlib import Path
import json

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222')
    page=browser.contexts[0].pages[0]
    errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
    page.locator('.search input').fill('meat computer')
    page.wait_for_function('document.querySelectorAll(".playlist-tile").length > 2')
    page.wait_for_timeout(900)
    def invoke(command,**args):return page.evaluate('(v)=>window.__TAURI_INTERNALS__.invoke(v.command,v.args)',{'command':command,'args':args})
    invoke('player_action',action='volume',value=0)
    page.locator('.artist-item').first.hover()
    icon_offset=page.locator('.artist-save').first.evaluate('''e=>{const b=e.getBoundingClientRect(),s=e.querySelector('svg').getBoundingClientRect();return Math.abs(b.x+b.width/2-s.x-s.width/2)+Math.abs(b.y+b.height/2-s.y-s.height/2)}''')
    assert icon_offset<1,icon_offset
    assert page.locator('body').evaluate('e=>getComputedStyle(e).userSelect')=='none'
    assert page.locator('.search input').evaluate('e=>getComputedStyle(e).userSelect')=='text'
    page.locator('.playlist-open').first.click()
    page.wait_for_function('document.querySelector(".track-heading-copy>span")?.textContent==="24 трека"')
    assert page.locator('.playlist-tile.selected').count()==1
    page.screenshot(path='.tools/screenshots/revised-playlist.png')
    page.locator('.context-back').click()
    page.locator('.playlist-tile').first.hover()
    page.locator('.tile-play').first.click()
    page.wait_for_timeout(4500)
    state=invoke('snapshot'); assert state['playing'] and len(state['queue'])==24,state.get('error')
    page.get_by_role('button',name='Полноэкранный плеер',exact=True).click()
    page.wait_for_timeout(650);page.screenshot(path='.tools/screenshots/revised-fullscreen.png')
    controls=page.locator('.player-options').bounding_box();center=page.evaluate('innerWidth/2')
    assert abs(controls['x']+controls['width']/2-center)<2
    page.keyboard.press('Escape')
    page.get_by_role('button',name='Мини-плеер',exact=True).click()
    page.wait_for_timeout(500);page.screenshot(path='.tools/screenshots/revised-mini.png')
    assert page.evaluate('innerWidth===430 && innerHeight===260')
    page.keyboard.press('Escape')
    invoke('player_action',action='toggle',value=None)
    assert not errors,errors
    report={'playlist_open':'24 tracks','playlist_one_click_play':True,'artist_icon_center_error_px':icon_offset,'text_selection':'inputs only','fullscreen_controls':'centered','mini_size':'430x260','page_errors':errors}
    Path('.tools/layout-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print(json.dumps(report,indent=2),flush=True)
    browser.close()
