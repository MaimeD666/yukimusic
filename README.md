# yukicloud

Неофициальный SoundCloud-плеер для **Windows 10/11 · x64**.

**v0.2** · [Установщик](https://github.com/MaimeD666/yukimusic/releases/download/v0.2/yukicloud_0.2.0_x64-setup.exe) · [Portable](https://github.com/MaimeD666/yukimusic/releases/download/v0.2/yukicloud_0.2.0_x64-portable.zip) · [Релиз](https://github.com/MaimeD666/yukimusic/releases/tag/v0.2)

![yukicloud](docs/screenshots/redesign-dark.png)

[Светлая тема](docs/screenshots/redesign-light.png)

- Поиск, любимые треки, плейлисты и артисты.
- Очередь с перетаскиванием, перемешивание и повтор.
- Обычный, полноэкранный и мини-плеер.
- 10-полосный эквалайзер, спектр и бас-режим.
- Две темы и цвета плеера по обложке.

Коллекция хранится локально. Входа в аккаунт и офлайн-загрузки нет.

<details>
<summary>Плееры, очередь и эквалайзер</summary>

**Полноэкранный плеер**

![Полноэкранный плеер](docs/screenshots/fullscreen.jpg)

**Мини-плеер**

![Мини-плеер](docs/screenshots/mini.png)

**Очередь**

![Очередь](docs/screenshots/queue.jpg)

**Эквалайзер**

![Эквалайзер](docs/screenshots/equalizer.jpg)

</details>

**Бас-режим · Boss — Lil Pump, 0:51–0:59**

![Бас-режим](docs/screenshots/bass.webp)

## Управление

`Пробел` — пауза · `Ctrl K` — поиск · `F` — полный экран · `M` — мини-плеер · `Q` — очередь · `B` — бас-режим.

Крестик сворачивает в трей. Выход — через меню трея.

<details>
<summary>Сборка из исходников</summary>

Node 22+, Rust MSVC, Visual Studio Build Tools (C++ и Windows SDK), Python 3.11+.

```powershell
npm ci
python scripts/prepare-ffmpeg.py
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/build.ps1
powershell -NoProfile -ExecutionPolicy Bypass -File scripts/package.ps1
```

Готовые файлы — в `release/`.

</details>

[Тестирование](docs/testing.md) · [Замеры](docs/measurements.json) · [Лицензии компонентов](src-tauri/resources/THIRD-PARTY.txt)
