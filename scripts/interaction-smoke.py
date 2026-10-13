"""Regressions reported during hands-on testing: playback, hover, geometry, FFT display.
Runs in an isolated browser with the native bridge fixture; never touches a user profile.
"""
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
import json, sys

report = {}
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':1280, 'height':850})
    page.add_init_script(path=str(Path(__file__).parent/'fixtures/native-ui.js'))
    errors = []
    page.on('pageerror', lambda e: errors.append(str(e)))
    page.goto(sys.argv[1] if len(sys.argv)>1 else 'http://127.0.0.1:1420')
    expect(page.locator('.track-row')).not_to_have_count(0)
    nav = page.locator('.primary-nav')
    nav.get_by_role('button', name='Артисты').click()
    page.wait_for_timeout(350)

    def rects(selector):
        return page.locator(selector).evaluate_all('es=>es.map(e=>{const r=e.getBoundingClientRect();return [r.x,r.y,r.width,r.height]})')

    def stable(before, after):
        assert len(before)==len(after), (before,after)
        assert all(abs(a-b)<.1 for old,new in zip(before,after) for a,b in zip(old,new)), (before,after)

    card = page.locator('[data-artist-id="101"]')
    targets = '.artist-item .collection-art, .artist-item .card-play, .artist-item strong'
    before = rects(targets)
    card.locator('.collection-open').hover()
    page.wait_for_timeout(350)
    stable(before, rects(targets))
    card.locator('.card-play').hover()
    page.wait_for_timeout(350)
    stable(before, rects(targets))
    assert page.locator('.card-more').count()==0
    report['hover_does_not_move_artwork_text_or_buttons']=True

    local_filter = page.get_by_role('textbox',name='Фильтр коллекции')
    local_filter.fill('Луна')
    card.locator('.card-play').click()
    expect(card.locator('.card-play')).to_have_attribute('aria-busy','true')
    page.wait_for_function('window.__uiTest.state.queue.length===16 && window.__uiTest.state.playing')
    expect(page.locator('h1')).to_have_text('Артисты')
    expect(local_filter).to_have_value('Луна')
    expect(card.locator('.card-play')).to_have_attribute('aria-busy','false')
    assert page.evaluate('new Set(window.__uiTest.state.queue.map(e=>e.track.id)).size')==16
    # The keyboard context menu uses the same playback path.
    card.locator('.collection-open').focus()
    page.keyboard.press('Shift+F10')
    expect(page.get_by_role('menu')).to_be_visible()
    page.get_by_role('menuitem', name='Слушать', exact=True).click()
    page.wait_for_timeout(300)
    expect(page.locator('h1')).to_have_text('Артисты')
    report['artist_play_keeps_view_filter_and_loads_unique_tracks']=True

    # Start another request before the artist's second page returns.
    page.evaluate('window.__uiTest.calls.length=0')
    card.locator('.card-play').evaluate('e=>e.click()')
    nav.get_by_role('button', name='Плейлисты').evaluate('e=>e.click()')
    page.locator('[data-playlist-id="202"] .card-play').evaluate('e=>e.click()')
    page.wait_for_function('window.__uiTest.state.queue.length===12')
    page.wait_for_timeout(350)
    expect(page.locator('h1')).to_have_text('Плейлисты')
    assert page.evaluate('window.__uiTest.calls.filter(c=>c.command==="player_action" && c.args.action==="play").length')==1
    report['latest_play_request_wins_without_navigation']=True

    # Pausing while a collection loads must not let its late response resume music.
    nav.get_by_role('button',name='Артисты').click()
    page.evaluate('window.__uiTest.calls.length=0')
    page.locator('[data-artist-id="101"] .card-play').evaluate('e=>e.click()')
    page.locator('.play-button').evaluate('e=>e.click()')
    page.wait_for_timeout(400)
    assert not page.evaluate('window.__uiTest.state.playing')
    assert page.evaluate('window.__uiTest.calls.filter(c=>c.command==="player_action" && c.args.action==="play").length')==0
    report['pause_cancels_pending_collection_play']=True

    nav.get_by_role('button', name='Плейлисты').click()
    page.evaluate('''() => {
      const invoke=window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke=(command,args)=>{
        if(command==='playlist_tracks' && window.__collectionResponse){
          if(window.__collectionResponse==='error') return Promise.reject(new Error('Тестовая ошибка загрузки'));
          const tracks=window.__collectionResponse==='empty'?[]:[{...window.__uiTest.library.tracks[0],available:false}];
          return Promise.resolve({tracks,next:null,total:tracks.length});
        }
        return invoke(command,args);
      };
    }''')
    queue_before=page.evaluate('window.__uiTest.state.queue')
    for response,message in [('error','Тестовая ошибка загрузки'),('empty','Нет доступных'),('unavailable','Нет доступных')]:
        page.evaluate('value=>window.__collectionResponse=value',response)
        page.locator('[data-playlist-id="201"] .card-play').click()
        expect(page.locator('.toast')).to_contain_text(message)
        expect(page.locator('[data-playlist-id="201"] .card-play')).to_have_attribute('aria-busy','false')
        expect(page.locator('h1')).to_have_text('Плейлисты')
        assert page.evaluate('window.__uiTest.state.queue')==queue_before
        assert not page.evaluate('window.__uiTest.state.playing')
    page.evaluate('window.__collectionResponse=null')
    page.locator('[data-playlist-id="201"] .card-play').click()
    page.wait_for_function('window.__uiTest.state.playing')
    report['failed_empty_and_unavailable_collections_preserve_queue_and_allow_retry']=True

    page.keyboard.press('Control+k')
    search = page.get_by_role('textbox',name='Поиск музыки')
    search.fill('unique search phrase')
    expect(page.locator('.track-title').first).to_contain_text('unique search phrase')
    expect(page.locator('.view-header')).to_have_count(0)
    assert 'Результаты для' not in page.locator('main').inner_text()
    page.locator('.search-tabs').get_by_role('button',name='Плейлисты').click()
    page.locator('[data-playlist-id="201"] .card-play').click()
    page.wait_for_timeout(180)
    expect(search).to_have_value('unique search phrase')
    expect(page.locator('.playlist-tile')).to_have_count(2)
    expect(page.locator('.view-header')).to_have_count(0)
    report['search_has_no_repeated_heading_and_play_keeps_results']=True

    nav.get_by_role('button',name='Любимые').click()
    for theme in ['dark','light']:
        if page.locator('html').get_attribute('data-theme')!=theme:
            page.locator('.theme-toggle').click()
        for width,height in [(800,620),(1024,768),(1280,850),(1920,1080)]:
            page.set_viewport_size({'width':width,'height':height})
            for panel in [False,True]:
                if panel: page.get_by_role('button',name='Открыть очередь').click()
                page.wait_for_timeout(320)
                a,b = rects('.settings-link, .theme-toggle')
                assert a[3]==b[3]==40,(width,theme,a,b)
                if abs(a[1]-b[1])>1: assert a[2]==b[2]==40,(a,b)
                else: assert abs(a[1]-b[1])<.1,(a,b)
                if panel:
                    before=rects('.queue-item:first-child, .queue-item:first-child button')
                    page.locator('.queue-item').first.hover()
                    page.wait_for_timeout(230)
                    stable(before,rects('.queue-item:first-child, .queue-item:first-child button'))
                    page.get_by_role('button',name='Закрыть очередь').click()
                    page.wait_for_timeout(300)
    report['sidebar_geometry_and_stationary_queue_in_both_themes_four_sizes']=True

    page.set_viewport_size({'width':1280,'height':850})
    page.get_by_role('button',name='Полноэкранный плеер',exact=True).click()
    page.wait_for_timeout(350)
    fixed='.player-art, .player-track, .player-center, .player-options, .player-top, .player-spectrum'
    before=rects(fixed)
    toggle=page.get_by_role('button',name='Переключить визуалайзер')
    for _ in range(4):
        toggle.click()
        stable(before,rects(fixed))
        page.wait_for_timeout(260)
        stable(before,rects(fixed))
    report['fullscreen_visualizer_never_changes_layout']=True

    canvas=page.locator('.player-spectrum canvas')
    # Distinct bass/treble signals must produce peaks in distinct positions,
    # and equal energy at either edge must remain equally visible.
    def signal(bin):
        page.evaluate('''bin=>{
          clearInterval(window.__signalTimer);
          window.__signalTimer=setInterval(()=>window.__uiTest.spectrum(Array.from({length:48},(_,i)=>i === bin ? .9 : 0)),20);
        }''',bin)
        page.wait_for_timeout(550)
        return canvas.evaluate('''c=>{const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data;
          let top=c.height,xTop=0;for(let y=0;y<c.height-3;y++)for(let x=0;x<c.width;x++){
            if(d[(y*c.width+x)*4+3]>60 && y<top){top=y;xTop=x}}
          return {top,x:xTop/c.width,height:c.height};}''')
    low=signal(3)
    high=signal(44)
    assert low['x']<.15 and high['x']>.85,(low,high)
    assert abs(low['top']-high['top'])<=2,(low,high)
    assert low['top']<low['height']*.3,(low,high)
    page.evaluate('clearInterval(window.__signalTimer)')
    report['spectrum_preserves_frequency_peaks_without_fixed_shape']=True
    page.keyboard.press('Escape')
    page.get_by_role('button',name='Настройки',exact=True).click()
    page.wait_for_timeout(350)
    fixed='.player-art, .player-track, .player-center, .player-options'
    before=rects(fixed)
    page.get_by_role('switch',name='Визуализация музыки').click()
    page.wait_for_timeout(300)
    stable(before,rects(fixed))
    report['dock_visualizer_keeps_layout']=True
    assert not errors, errors
    report['page_errors']=errors
    browser.close()
Path('.tools/interaction-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
print(json.dumps(report,indent=2))
