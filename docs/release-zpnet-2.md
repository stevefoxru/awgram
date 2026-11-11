# Безопасный выпуск ZPNet 2.0

Этот runbook описывает следующий major-релиз существующей системы. Он не
разрешает автоматическую миграцию production-БД и не заменяет ручное решение о
выпуске.

## 1. Release gate до тега

1. Рабочее дерево должно быть чистым, а релизная версия — иметь формат SemVer.
2. `Cargo.toml`, заголовок версии в `CHANGELOG.md` и будущий тег `vX.Y.Z`
   должны совпадать. Версия 2.0 выпускается как `2.0.0`; пробные сборки — как
   `2.0.0-rc.N`.
3. На Linux выполнить:

   ```bash
   ./scripts/release-preflight.sh --version X.Y.Z
   ```

   Скрипт проверяет lockfile и зависимости через `cargo-deny`, форматирование,
   Clippy, Rust-тесты, routing-тесты, JavaScript, shellcheck и release-сборку
   всех бинарников. Он ничего не публикует и не обращается к production.
4. Сначала разрешается только ручная dry-run сборка существующего workflow.
   Создание тега и GitHub Release — отдельное подтверждаемое действие.

## 2. Production preflight без изменений БД

Перед обновлением сохранить вывод:

```bash
sudo /usr/local/bin/awgram --schema-status
sudo /usr/local/bin/awgram --check-db > /root/awgram-db-before.json
sudo systemctl is-active awgram
sudo systemctl show awgram -p NRestarts --value
```

Новый `awgram-setup update` до замены бинарника сам запускает у скачанного
бинарника `--schema-status` и `--check-db` против действующей БД. Несовместимая
схема, повреждение SQLite, нарушение внешних ключей или доменная аномалия
останавливают обновление. Installer не запускает `--migrate-db`.

Перед одобренным production-релизом создать отдельную SQLite-consistent копию:

```bash
db="$(sudo /usr/local/bin/awgram --database-path)"
backup="/var/lib/awgram/backups/release-$(date -u +%Y%m%dT%H%M%SZ).db"
sudo install -d -m 700 /var/lib/awgram/backups
sudo sqlite3 "$db" ".backup '$backup'"
sudo sqlite3 "$backup" 'PRAGMA quick_check'
sudo chmod 600 "$backup"
```

Результат `quick_check` должен быть ровно `ok`. Путь backup записывается в
журнал релиза.

## 3. Ручной production deploy

1. Зафиксировать предыдущую версию, контрольные суммы и путь backup.
2. Выполнить обновление только после ручного подтверждения:

   ```bash
   sudo awgram-setup update --version vX.Y.Z --yes
   ```

3. Не выполнять migration-команду, если schema preflight сообщил
   несовместимость. Сначала нужен отдельный план миграции и отдельное разрешение.

## 4. Проверка после запуска

```bash
sudo systemctl is-active awgram
sudo systemctl show awgram -p NRestarts --value
sudo /usr/local/bin/awgram --schema-status
sudo /usr/local/bin/awgram --check-db > /root/awgram-db-after.json
sudo journalctl -u awgram --since '-10 min' --no-pager
```

Критерии успеха: служба непрерывно активна, `NRestarts=0`, схема совместима,
DB Doctor завершился кодом 0. Затем вручную проверить ровно по одному безопасному
сценарию: `/start`, открытие «Моих ключей», вход в веб-кабинет, скачивание
существующей конфигурации и открытие формы покупки без создания платежа.

## 5. Откат

Если схема не менялась:

```bash
sudo /usr/local/libexec/awgram-updatectl rollback
sudo /usr/local/bin/awgram --schema-status
sudo /usr/local/bin/awgram --check-db
```

Если отдельно согласованная миграция всё же выполнялась, одного отката бинарника
недостаточно. Нужно остановить сервис, сохранить проблемную БД для расследования,
восстановить именно проверенный pre-release backup, установить предыдущий
бинарник и только затем запустить службу. Такой откат заранее репетируется на
копии production-БД.

## 6. Stop conditions

Релиз прекращается без попытки «починить на ходу», если не совпадает версия,
нет проверенного backup, DB Doctor неуспешен, схема несовместима, служба
перезапускается, одновременно обнаружены два poller-процесса или smoke test
затрагивает существующие подписки, платежи либо VPN-ключи.
