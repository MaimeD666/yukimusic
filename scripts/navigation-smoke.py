"""Isolated navigation/layout regression against a served production build.
No user profile, audio device, or external service is changed.
Usage: python scripts/navigation-smoke.py [http://127.0.0.1:1420]
Requires Python Playwright and Microsoft Edge.
"""
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
import json, sys

url = sys.argv[1] if len(sys.argv) > 1 else 'http://127.0.0.1:1420'
report = {}
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':1280,'height':850})
    page.add_init_script(path=str(Path(__file__).parent / 'fixtures/native-ui.js'))
    errors=[]
    page.on('pageerror', lambda e: errors.append(str(e)))
    page.goto(url)
    expect(page.locator('h1')).to_have_text('Любимые треки')
    expect(page.locator('html')).to_have_attribute('data-theme','dark')
    expect(page.locator('.collection-card')).to_have_count(0)
    expect(page.locator('.player')).to_be_visible()
    nav = page.locator('.primary-nav')
    query = page.get_by_role('textbox', name='Фильтр коллекции')
    query.fill('  ОБЛАКА ')
    expect(page.locator('.track-row')).to_have_count(1)
    page.locator('.track-row .track-artist-column').click()
    expect(page.locator('h1')).to_have_text('Луна')
    expect(page.locator('.view-heading p')).to_contain_text('16 треков')
    page.get_by_role('button',name='Назад',exact=True).click()
    expect(query).to_have_value('  ОБЛАКА ')
    expect(page.locator('.track-row')).to_have_count(1)
    page.get_by_role('button',name='Вперёд',exact=True).click()
    expect(page.locator('h1')).to_have_text('Луна')
    page.keyboard.press('Alt+ArrowLeft')
    page.get_by_role('button',name='Сбросить фильтр',exact=True).click()
    page.get_by_label('Сортировка треков').select_option('duration')
    assert page.evaluate('window.__uiTest.library.tracks[0].title') == 'Облака'
    report['filter_sort_history'] = True

    page.locator('.track-scroll').evaluate('(e)=>e.scrollTop=2100')
    page.wait_for_timeout(50)
    # Capture where the click actually occurs; browser automation may scroll a
    # locator into view while the previous navigation transition is settling.
    page.locator('.track-scroll').evaluate("e=>e.addEventListener('pointerdown',()=>window.__leavingScroll=e.scrollTop,{once:true})")
    page.locator('.track-row .track-artist-column').nth(6).click()
    before_scroll = page.evaluate('window.__leavingScroll')
    expect(page.locator('.view-kicker')).to_have_text('Артист')
    page.get_by_role('button',name='Назад',exact=True).click()
    page.wait_for_function('(expected)=>Math.abs(document.querySelector(".track-scroll").scrollTop-expected)<2',arg=before_scroll)
    assert abs(page.locator('.track-scroll').evaluate('(e)=>e.scrollTop') - before_scroll) < 2
    report['scroll_restored'] = True

    nav.get_by_role('button',name='Плейлисты').click()
    expect(page.locator('h1')).to_have_text('Плейлисты')
    expect(nav.get_by_role('button',name='Плейлисты')).to_have_attribute('aria-current','page')
    page.get_by_role('button',name='Слушать Ночная сторона').click()
    page.wait_for_function('window.__uiTest.state.playing && window.__uiTest.state.queue.length===12')
    expect(page.locator('h1')).to_have_text('Плейлисты')
    page.locator('[data-playlist-id="201"] .collection-open').click()
    expect(page.locator('h1')).to_have_text('Ночная сторона')
    report['one_click_playlist'] = True

    page.get_by_role('button',name='Открыть очередь').click()
    expect(page.locator('.side-panel')).to_be_visible()
    expect(page.locator('.panel-scrim')).to_have_count(0)
    page.locator('.track-row').first.get_by_role('button',name='В конец очереди').click()
    page.wait_for_function('window.__uiTest.state.queue.length===13')
    page.locator('.queue-list').evaluate('(e)=>e.scrollTop=9999')
    duplicate = page.evaluate('window.__uiTest.state.queue.at(-1).key')
    page.locator(f'[data-queue-key="{duplicate}"]').get_by_role('button',name='Удалить Облака из очереди').click()
    page.wait_for_function('window.__uiTest.state.queue.length===12')
    assert page.evaluate('window.__uiTest.state.queue[0].track.title') == 'Облака'
    page.get_by_role('button',name='Закрыть очередь').click()
    report['queue_panel_and_duplicate_removal'] = True

    page.keyboard.press('Control+k')
    search = page.get_by_role('textbox',name='Поиск музыки')
    expect(search).to_be_focused()
    search.fill('slow')
    page.wait_for_timeout(350)
    search.fill('new')
    expect(page.locator('.track-title').first).to_contain_text('new')
    page.wait_for_timeout(600)
    expect(page.locator('.track-title').first).to_contain_text('new')
    page.locator('.search-tabs').get_by_role('button',name='Артисты').click()
    expect(page.locator('.artist-item')).to_have_count(2)
    page.locator('.search-tabs').get_by_role('button',name='Плейлисты').click()
    expect(page.locator('.playlist-tile')).to_have_count(2)
    page.locator('.search-tabs').get_by_role('button',name='Треки').click()
    page.locator('.track-scroll').evaluate('(e)=>e.scrollTop=99999')
    page.wait_for_function('window.__uiTest.calls.some(c=>c.command==="search_more")')
    report['search_tabs_stale_responses_pagination'] = True

    search.fill('slow')
    page.wait_for_timeout(350)
    nav.get_by_role('button',name='Любимые').click()
    page.wait_for_timeout(650)
    expect(page.locator('h1')).to_have_text('Любимые треки')
    expect(page.locator('.track-title').first).to_have_text('Облака')
    report['search_does_not_replace_library'] = True
    nav.get_by_role('button',name='Поиск').click()
    expect(page.locator('.track-title').first).to_contain_text('slow')
    page.evaluate('window.__uiTest.failSearch=true')
    search.fill('offline')
    expect(page.locator('.inline-error')).to_contain_text('Тестовая ошибка сети')
    page.evaluate('window.__uiTest.failSearch=false')
    page.locator('.inline-error').get_by_role('button',name='Повторить').click()
    expect(page.locator('.track-title').first).to_contain_text('offline')
    report['interrupted_search_resumes_and_errors_retry'] = True
    nav.get_by_role('button',name='Любимые').click()

    page.get_by_role('button',name='Включить светлую тему').click()
    expect(page.locator('html')).to_have_attribute('data-theme','light')
    page.reload()
    expect(page.locator('html')).to_have_attribute('data-theme','light')
    page.get_by_role('button',name='Настройки',exact=True).click()
    page.get_by_role('button',name='Лунная').click()
    expect(page.locator('html')).to_have_attribute('data-theme','dark')
    page.locator('.player-artist').click()
    expect(page.locator('.view-kicker')).to_have_text('Артист')
    report['theme_persistence_and_player_navigation'] = True

    page.get_by_role('button',name='Эквалайзер',exact=True).click()
    page.get_by_role('button',name='Воздух',exact=True).click()
    page.wait_for_function('window.__uiTest.state.eq[8]===4 && window.__uiTest.state.eqEnabled')
    page.get_by_role('button',name='Закрыть эквалайзер').click()
    page.locator('.play-button').click()
    page.wait_for_function('window.__uiTest.state.playing')
    seek = page.get_by_role('slider',name='Позиция воспроизведения')
    seek.fill('40')
    seek.dispatch_event('change')
    page.wait_for_function('window.__uiTest.state.position===40')
    expect(seek).to_have_value('40')
    report['equalizer_playback_seek'] = True

    def geometry():
        result = page.evaluate('''() => {
          const selectors=['.application','.app-body','.work-area','main','.player','.side-panel','.player-options','.list-toolbar','.eq-bands','.eq-bands label','.eq-bands input','.panel-note'];
          return selectors.flatMap(selector=>{const e=document.querySelector(selector);if(!e)return [];
            const r=e.getBoundingClientRect();return r.left < -1 || r.right > innerWidth+1 || r.bottom > innerHeight+1 || e.scrollWidth > e.clientWidth+2 ? [selector] : [];});
        }''')
        assert not result,result
    nav.get_by_role('button',name='Любимые').click()
    expect(page.locator('.side-panel')).to_have_count(0)
    for width,height in [(800,620),(1024,768),(1280,850),(1920,1080)]:
        page.set_viewport_size({'width':width,'height':height})
        geometry()
        for label,close in [('Открыть очередь','Закрыть очередь'),('Эквалайзер','Закрыть эквалайзер')]:
            page.get_by_role('button',name=label,exact=True).click()
            page.wait_for_timeout(300)
            geometry()
            page.get_by_role('button',name=close).click()
            page.wait_for_timeout(300)
    report['four_window_sizes_with_both_panels'] = True
    page.set_viewport_size({'width':1280,'height':850})
    page.get_by_role('button',name='Полноэкранный плеер',exact=True).click()
    expect(page.locator('.player.immersive')).to_be_visible()
    geometry()
    page.keyboard.press('Escape')
    page.get_by_role('button',name='Мини-плеер',exact=True).click()
    page.set_viewport_size({'width':430,'height':260})
    expect(page.locator('.player.mini-player')).to_be_visible()
    geometry()
    page.keyboard.press('Escape')
    page.set_viewport_size({'width':1280,'height':850})
    report['expanded_and_mini_modes'] = True
    report['page_errors'] = errors
    assert not errors,errors
    browser.close()
out=Path('.tools/redesign-navigation-report.json')
out.parent.mkdir(exist_ok=True)
out.write_text(json.dumps(report,indent=2),encoding='utf8')
print(json.dumps(report,indent=2))
