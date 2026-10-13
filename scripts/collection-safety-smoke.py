"""Cover colors, deliberate removal, and app tooltips against the UI fixture."""
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
import json, sys
report={}
with sync_playwright() as p:
    browser=p.chromium.launch(channel='msedge',headless=True)
    page=browser.new_page(viewport={'width':1280,'height':850})
    page.add_init_script(path=str(Path(__file__).parent/'fixtures/native-ui.js'))
    errors=[]
    page.on('pageerror',lambda e:errors.append(str(e)))
    page.on('dialog',lambda dialog: (_ for _ in ()).throw(AssertionError('Unexpected browser dialog: '+dialog.message)))
    page.goto(sys.argv[1] if len(sys.argv)>1 else 'http://127.0.0.1:1420')
    expect(page.locator('.track-row')).not_to_have_count(0)
    assert page.locator('body [title]').count()==0
    page.locator('.primary-nav').get_by_role('button',name='Артисты').click()
    card=page.locator('[data-artist-id="101"]')
    card.locator('.artist-main').hover()
    page.wait_for_timeout(800)
    expect(page.get_by_role('tooltip')).to_have_count(0)
    expect(card.locator('.card-more, .card-save')).to_have_count(0)
    card.click(button='right')
    expect(page.get_by_role('menu')).to_be_visible()
    assert len(page.evaluate('window.__uiTest.library.artists'))==2
    page.get_by_role('menuitem',name='Убрать из коллекции').click()
    expect(page.get_by_role('alertdialog')).to_be_visible()
    expect(page.get_by_role('button',name='Отмена',exact=True)).to_be_focused()
    assert len(page.evaluate('window.__uiTest.library.artists'))==2
    page.keyboard.press('Escape')
    expect(page.get_by_role('alertdialog')).to_have_count(0)
    assert len(page.evaluate('window.__uiTest.library.artists'))==2
    card.click(button='right')
    page.get_by_role('menuitem',name='Убрать из коллекции').click()
    page.get_by_role('alertdialog').get_by_role('button',name='Убрать из коллекции').click()
    expect(card).to_have_count(0)
    page.wait_for_function('window.__uiTest.library.artists.length===1')
    assert len(page.evaluate('window.__uiTest.library.tracks'))==90
    assert len(page.evaluate('window.__uiTest.state.queue'))==3
    report['artist_removal_is_separate_confirmed_and_cancellable']=True
    page.locator('.primary-nav').get_by_role('button',name='Плейлисты').click()
    page.locator('.playlist-tile').first.click(button='right')
    page.get_by_role('menuitem',name='Убрать из коллекции').click()
    expect(page.get_by_role('alertdialog')).to_be_visible()
    page.get_by_role('button',name='Отмена',exact=True).click()
    expect(page.get_by_role('alertdialog')).to_have_count(0)
    assert len(page.evaluate('window.__uiTest.library.playlists'))==2
    report['context_menu_cannot_bypass_confirmation']=True
    page.get_by_role('button',name='Открыть очередь').hover()
    page.wait_for_timeout(350)
    expect(page.get_by_role('tooltip')).to_have_count(0)
    expect(page.get_by_role('tooltip')).to_be_visible()
    page.get_by_role('button',name='Открыть очередь').click()
    expect(page.get_by_role('tooltip')).to_have_count(0)
    page.get_by_role('button',name='Закрыть очередь').click()
    report['delayed_tooltips_only_for_controls_and_no_native_titles']=True

    page.evaluate('''() => {
      const original=window.__TAURI_INTERNALS__.invoke;
      window.__TAURI_INTERNALS__.invoke=(cmd,args)=>{
        if(cmd==='artwork' && args.url.startsWith('fixture:')) {
          const c=document.createElement('canvas');c.width=c.height=32;
          const ctx=c.getContext('2d');ctx.fillStyle=args.url==='fixture:red'?'#cf3020':'#2040cd';ctx.fillRect(0,0,32,32);return Promise.resolve(c.toDataURL());
        }return original(cmd,args);
      };
      window.__playColor=color=>original('player_action',{action:'play',value:{tracks:[{...window.__uiTest.library.tracks[0],id:color==='red'?400:401,artwork:'fixture:'+color}],index:0}});
    }''')
    def color():return page.locator('.player').evaluate('e=>getComputedStyle(e).getPropertyValue("--player-bg")')
    page.evaluate("window.__playColor('red')")
    page.wait_for_timeout(550)
    red=color()
    page.evaluate("window.__playColor('blue')")
    page.wait_for_timeout(550)
    blue=color()
    assert red!=blue,(red,blue)
    page.get_by_role('button',name='Включить светлую тему').click()
    page.wait_for_timeout(400)
    light_blue=color()
    page.evaluate("window.__playColor('red')")
    page.wait_for_timeout(550)
    light_red=color()
    assert light_red!=light_blue and light_blue!=blue,(red,blue,light_red,light_blue)
    report['dock_follows_cover_in_both_themes']=True
    assert not errors,errors
    report['page_errors']=errors
    browser.close()
Path('.tools/collection-safety-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
print(json.dumps(report,indent=2))
