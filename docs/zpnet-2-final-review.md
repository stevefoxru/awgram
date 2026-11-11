# ZPNet 2.0: итоговое code review перед RC

Дата: 2026-11-11. Диапазон review: `v1.98.0..codex/zpnet-a08-final-review`.
Документ фиксирует локальный результат и не разрешает push, публикацию тега,
production deploy или изменение БД.

## Результат

В ходе финального review найден и исправлен один дополнительный дефект высокой
важности: lock-файл, созданный root с правами `0600`, мог заблокировать запуск
того же приложения под hardened-пользователем. Lock теперь создаётся с
предсказуемыми правами независимо от `umask`, открывается hardened-процессом
read-only и защищён от symlink/hardlink/FIFO-подмены в общем `/tmp`.

Других подтверждённых критических или высоких дефектов в изменённом диапазоне
не найдено. Это не заменяет Linux CI и испытания на копии production-БД.

## Проверенные инварианты

- обычный запуск не создаёт и не мигрирует БД;
- каждый migration batch и номер схемы фиксируются одной транзакцией;
- backup миграции проверяется до изменения схемы;
- DB Doctor открывает SQLite read-only и проверяет schema, quick check,
  внешние ключи и доменные аномалии;
- пополнение меняет статус и ledger атомарно и не допускает повторное
  зачисление;
- одинаковый Telegram token не может одновременно использоваться двумя
  процессами на одном сервере;
- installer проверяет staged binary до замены и не запускает миграцию;
- покупка, пополнение и продление сохраняют ID заявки и путь отправки
  подтверждения после повторного открытия кабинета;
- имена скачиваемых конфигураций уникальны и безопасны для HTTP-заголовка;
- версия отображается в административном интерфейсе без автоматической смены.

## Локально выполненные проверки

- `git diff --check`;
- `cargo fmt --all -- --check`;
- `node --check frontend/app.js`;
- `bash -n` для изменённых installer/release scripts;
- компиляция Rust test targets для `x86_64-unknown-linux-musl` через Zig.

Нативный Windows Clippy не запускается без MSVC `link.exe`. Docker также
недоступен в текущей среде. Поэтому Clippy, выполнение Rust-тестов, shellcheck,
`cargo-deny`, installer integration test и arm64 build остаются Linux gates, а
не считаются пройденными по локальному review.

## Точный состав будущего RC

В review-ветку входят восемь независимых этапов:

1. A01 — явное управление схемой и backup миграции;
2. A02 — read-only DB Doctor;
3. A03 — атомарное подтверждение пополнения;
4. A04 — single-instance Telegram lock;
5. A05 — исправление критических checkout/renewal UX-сценариев;
6. A06 — guarded installer preflight и release runbook;
7. A07 — Go/No-Go матрица;
8. A08 — исправление cross-user и `/tmp` security дефекта lock-файла.

Первый допустимый version commit должен содержать только:

- `Cargo.toml`: `1.98.0` → `2.0.0-rc.1`;
- синхронное обновление `Cargo.lock`;
- перенос текущего `Unreleased` в секцию `[2.0.0-rc.1]` с датой;
- ссылки на новую секцию changelog, если они используются внизу файла.

Смешивать с этим commit функциональные исправления запрещено. Сам version
commit создаётся только после отдельного подтверждения владельца.

## Оставшиеся обязательные gates

1. Review diff другим человеком.
2. Успешные Linux CI и dependency audit.
3. Dry-run release workflow для amd64 и arm64 с проверкой checksums.
4. DB Doctor и запуск RC на свежей изолированной копии production-БД.
5. Installer update/rollback test в disposable Linux-среде.
6. Ручной UX smoke-test Telegram WebView и desktop browser.
7. Проверка отсутствия legacy bot/container на production-host.
8. Проверенный backup и репетиция восстановления.

До закрытия этих gates статус остаётся **NO-GO для production**.
