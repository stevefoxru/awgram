# ZPNet Routing Manager и Automatic Failover

Реализация объединяет policy routing и failover в одном агенте. Второй Telegram-бот не запускается, Docker и AmneziaWG при переключении не перезапускаются.

## Безопасная схема

- Проверка выполняется через HTTP endpoint на tunnel peer и подтверждает ожидаемый публичный IP EXIT. Один handshake не считается здоровьем.
- После трёх ошибок EXIT выводится из работы; возврат происходит после четырёх успешных проверок.
- Когда оба EXIT недоступны, в таблицах устанавливается `blackhole default`: утечки через обычный маршрут RU исключены.
- Автоматический режим можно заблокировать вручную. Персональный режим клиента: `auto`, `main` или `nl`; при отказе выбранного EXIT используется живой резерв.
- Состояние пишется атомарно, изменения сериализованы `flock`, журнал хранится локально. Агент поддерживает больше двух EXIT.

## Поэтапное включение

1. На каждом EXIT установите `zpnet-exit-health.py` как systemd-службу, слушающую только tunnel IP:18081. Задайте `ZPNET_EXPECTED_PUBLIC_IP`.
2. На RU выполните `install.sh install-ru`, проверьте `/etc/zpnet-routing/config.json` и запустите `install.sh preflight`. Это не меняет маршруты.
3. Сохраните вывод `ip rule`, таблиц 100/200, `iptables-save` и `ipset save`. Убедитесь, что старый watchdog известен.
4. Только в согласованное окно выполните `install.sh activate --confirm`. Команда сохраняет резервную копию, отключает старый `zpnet-failover.timer` и включает единственный менеджер.
5. Проверьте normal, отказ Main, отказ NL, возврат, оба недоступны, ручную блокировку и персональные режимы. Откат: `install.sh rollback`.

## Подключение awgram

Создайте отдельный SSH-ключ без shell-доступа. На RU в `authorized_keys` используйте `command="/usr/local/libexec/zpnet-routingctl",no-port-forwarding,no-agent-forwarding,no-X11-forwarding,no-pty`. На сервере бота установите `awgram-routing-remote`, отдельный `known_hosts` и SSH config. В `/etc/awgram/config.toml`:

```toml
routing_control_command = "/usr/local/libexec/awgram-routing-remote"
```

Команды ограничены `status`, `dry-run`, `rollback`, переключателем автоматики и адресами `10.66.0.0/24`. Все административные действия остаются в существующем боте: **Админка → Управление EXIT**.

## Важные границы

Приложенные архивы использованы как прототипы, но их отдельный polling-бот и watchdog устанавливать не следует. Источники YouTube/GitHub задаются в `ipsets` конфигурации. `ipset-update --dry-run` проверяет URL, размер и публичные CIDR, а `ipset-update` применяет набор через временный set и атомарный `ipset swap`. Предыдущий набор остаётся под именем `<name>-previous` для быстрого отката; обновление намеренно не смешано с аварийным переключением маршрутов.
