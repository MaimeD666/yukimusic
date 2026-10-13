"""Verify the desktop layouts and playlist actions against the native application."""
from playwright.sync_api import sync_playwright
from pathlib import Path
import json

with sync_playwright() as p:
    browser=p.chromium.connect_over_cdp('http://127.0.0.1:9222')
    page=browser.contexts[0].pages[0]
    errors=[];page.on('pageerror',lambda e:errors.append(str(e)))
    page.locator('.search input').fill('meat computer')
    page.locator('.search-tabs').get_by_role('button',name='Артисты').click()
    page.wait_for_function('document.querySelectorAll(".artist-item").length > 0')
    page.wait_for_timeout(900)
    def invoke(command,**args):return page.evaluate('(v)=>window.__TAURI_INTERNALS__.invoke(v.command,v.args)',{'command':command,'args':args})
    invoke('player_action',action='volume',value=0)
    page.locator('.artist-item').first.hover()
    icon_offset=page.locator('.card-play').first.evaluate('''e=>{const b=e.getBoundingClientRect(),s=e.querySelector('svg').getBoundingClientRect();return Math.abs(b.x+b.width/2-s.x-s.width/2)+Math.abs(b.y+b.height/2-s.y-s.height/2)}''')
    assert icon_offset<1,icon_offset
    assert page.locator('body').evaluate('e=>getComputedStyle(e).userSelect')=='none'
    assert page.locator('.search input').evaluate('e=>getComputedStyle(e).userSelect')=='text'
    page.locator('.search-tabs').get_by_role('button',name='Плейлисты').click()
    page.wait_for_function('document.querySelectorAll(".playlist-tile").length > 0')
    title=page.locator('.playlist-open strong').first.inner_text()
    playlist_id=int(page.locator('.playlist-tile').first.get_attribute('data-playlist-id'))
    expected_count=len(invoke('playlist_tracks',id=playlist_id)['tracks'])
    page.locator('.playlist-open').first.click()
    page.wait_for_function('(count)=>document.querySelector(".track-table")?.getAttribute("aria-rowcount")===String(count+1)',arg=expected_count)
    assert page.locator('h1').inner_text()==title
    page.screenshot(path='.tools/screenshots/revised-playlist.png')
    page.get_by_role('button',name='Назад',exact=True).click()
    page.locator('.playlist-tile').first.hover()
    page.locator('.card-play').first.click()
    page.wait_for_timeout(4500)
    state=invoke('snapshot'); assert state['playing'] and len(state['queue'])==expected_count,state.get('error')
    assert page.locator('.playlist-tile').count()>0
    assert page.locator('.view-header').count()==0
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
    report={'playlist_tracks':expected_count,'playlist_one_click_play':True,'artist_icon_center_error_px':icon_offset,'text_selection':'inputs only','fullscreen_controls':'centered','mini_size':'430x260','page_errors':errors}
    Path('.tools/layout-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
    print(json.dumps(report,indent=2),flush=True)
    browser.close()
