"""Motion and audio-visual feedback in an isolated browser; no real audio/profile."""
from pathlib import Path
from playwright.sync_api import sync_playwright, expect
import json, sys

report = {}
with sync_playwright() as p:
    browser = p.chromium.launch(channel='msedge', headless=True)
    page = browser.new_page(viewport={'width':1280,'height':850})
    page.add_init_script(path=str(Path(__file__).parent/'fixtures/native-ui.js'))
    page.add_init_script('''window.__canvasFrames=0;const clear=CanvasRenderingContext2D.prototype.clearRect;
      CanvasRenderingContext2D.prototype.clearRect=function(...args){window.__canvasFrames++;return clear.apply(this,args)};''')
    errors=[]
    page.on('pageerror',lambda e:errors.append(str(e)))
    page.goto(sys.argv[1] if len(sys.argv)>1 else 'http://127.0.0.1:1420')
    expect(page.locator('.track-row')).not_to_have_count(0)
    page.wait_for_timeout(350)
    icon=page.locator('.settings-link svg path').evaluate('e=>{const b=e.getBBox();return {x:b.x,y:b.y,w:b.width,h:b.height}}')
    assert icon['x']>=1 and icon['y']>=1 and icon['x']+icon['w']<=23 and icon['y']+icon['h']<=23,icon
    assert abs(icon['x']+icon['w']/2-12)<.05 and abs(icon['y']+icon['h']/2-12)<.05,icon
    report['settings_icon_centered_and_unclipped']=True
    page.get_by_role('button',name='Настройки',exact=True).evaluate('(e)=>e.click()')
    page.wait_for_function("document.querySelector('.view-content').getAnimations().some(a=>a.playState==='running')")
    expect(page.locator('h1')).to_have_text('Настройки')
    assert 'Та же музыка' not in page.locator('body').inner_text()
    assert 'пространство музыки' not in page.locator('body').inner_text()
    page.wait_for_timeout(300)
    old=page.locator('html').evaluate('e=>getComputedStyle(e).getPropertyValue("--bg")')
    page.get_by_role('button',name='Облачная',exact=True).evaluate('(e)=>e.click()')
    page.wait_for_timeout(110)
    mid=page.locator('html').evaluate('e=>getComputedStyle(e).getPropertyValue("--bg")')
    page.wait_for_timeout(250)
    end=page.locator('html').evaluate('e=>getComputedStyle(e).getPropertyValue("--bg")')
    assert old!=mid and mid!=end,(old,mid,end)
    page.get_by_role('button',name='Лунная',exact=True).click()
    report['navigation_and_theme_interpolate']=True
    page.locator('.primary-nav').get_by_role('button',name='Любимые').click()
    page.wait_for_timeout(320)
    widths=page.evaluate('''async()=>{document.querySelector('[aria-label="Открыть очередь"]').click();
      const samples=[];const start=performance.now();while(performance.now()-start<340){await new Promise(requestAnimationFrame);samples.push(document.querySelector('.side-panel')?.getBoundingClientRect().width||0)}return samples}''')
    assert len([x for x in widths if 5<x<330])>=3,widths
    assert all(b>=a-1 for a,b in zip(widths,widths[1:])),widths
    page.get_by_role('button',name='Закрыть очередь').click()
    expect(page.locator('.side-panel')).to_have_count(0)
    report['panel_reserves_space_gradually']=True

    canvas=page.locator('.dock-spectrum canvas')
    expect(canvas).to_be_visible()
    page.wait_for_function("window.__uiTest.calls.some(c=>c.command==='visualization' && c.args.enabled)")
    def occupied_rows():
        return canvas.evaluate('''c=>{const d=c.getContext('2d').getImageData(0,0,c.width,c.height).data;let rows=0;
          for(let y=0;y<c.height;y++){for(let x=0;x<c.width;x++){if(d[(y*c.width+x)*4+3]>8){rows++;break}}}return rows}''')
    baseline=occupied_rows()
    page.locator('.play-button').click()
    page.evaluate('''window.__spectrumTimer=setInterval(()=>window.__uiTest.spectrum(Array.from({length:48},(_,i)=>.35+.55*Math.exp(-Math.pow((i-14)/8,2)))),25)''')
    page.wait_for_timeout(400)
    assert occupied_rows()>baseline+3,(baseline,occupied_rows())
    page.locator('.play-button').click()
    page.evaluate('clearInterval(window.__spectrumTimer)')
    page.wait_for_timeout(1900)
    assert occupied_rows()<=baseline+1,(baseline,occupied_rows())
    frames=page.evaluate('window.__canvasFrames')
    page.wait_for_timeout(250)
    assert page.evaluate('window.__canvasFrames')==frames
    report['wave_uses_spectrum_fades_on_pause_and_stops_idle_frames']=True
    page.get_by_role('button',name='Полноэкранный плеер',exact=True).click()
    expect(page.locator('.player-spectrum canvas')).to_be_visible()
    page.keyboard.press('Escape')
    page.emulate_media(reduced_motion='reduce')
    page.get_by_role('button',name='Настройки',exact=True).click()
    assert not page.locator('.view-content').evaluate('e=>e.getAnimations().some(a=>a.playState==="running")')
    page.get_by_role('switch',name='Визуализация музыки',exact=True).click()
    expect(page.locator('.dock-spectrum')).to_have_attribute('aria-hidden','true')
    expect(page.locator('.dock-spectrum')).to_have_css('opacity','0')
    page.wait_for_function("window.__uiTest.calls.filter(c=>c.command==='visualization').at(-1).args.enabled===false")
    report['expanded_wave_settings_and_reduced_motion']=True
    assert not errors,errors
    report['page_errors']=errors
    browser.close()
Path('.tools/motion-report.json').write_text(json.dumps(report,indent=2),encoding='utf8')
print(json.dumps(report,indent=2))
