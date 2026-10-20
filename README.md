# opal

Минималистичный Markdown-редактор: разметка видна только у элемента под курсором, колонка текста по центру, typewriter mode, автосохранение. GTK4, без веб-движка.

## Сборка

Debian: `sudo apt install libgtk-4-dev pkg-config` · FreeBSD: `pkg install gtk4 rust pkgconf`

```
cargo build --release
cp target/release/opal ~/.local/bin/
```

## Использование

`opal файл.md` — открыть (если файла нет, он появится при первом сохранении); `opal` — новый документ.

| Клавиши | |
|---|---|
| Ctrl+S | сохранить сейчас (для нового документа — выбрать имя) |
| Ctrl+O / Ctrl+N | открыть / новый документ |
| Ctrl+T | typewriter mode вкл/выкл |
| Ctrl+Q | закрыть |
| Ctrl+клик | открыть ссылку |

Сохранение автоматическое: через секунду после паузы и при закрытии.

## Настройки

`~/.config/opal/config`:

```
column_width = 70        # ширина колонки в символах
font = Serif 14
mono_font = Monospace 12
typewriter = true
theme = light            # light | dark
```

Рисует процессором (рендерер GTK `cairo`): так окно появляется быстрее и не загружаются драйверы видеокарты. Другой рендерер можно выбрать как обычно в GTK: `GSK_RENDERER=vulkan opal`.

## Тесты

`cargo test --lib` — логика; `cargo test --test gtk_editor` и `cargo test --test gtk_window` открывают окна и требуют дисплей.
