use teloxide::types::{InlineKeyboardButton, InlineKeyboardMarkup, KeyboardButton, KeyboardMarkup};

use crate::i18n::{self, Lang};
use crate::vpn::model::{format_handshake_compact, Client, ClientFilter};
use crate::vpn::BackupFile;

fn cb(text: &str, data: &str) -> InlineKeyboardButton {
    InlineKeyboardButton::callback(text.to_string(), data.to_string())
}

pub fn profile_menu(portal_enabled: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if portal_enabled {
        rows.push(vec![cb("🌐 Открыть веб-кабинет", "portal")]);
    }
    rows.push(vec![
        cb("🔑 Мои ключи", "mykeys"),
        cb("💰 Баланс", "balance"),
    ]);
    rows.push(vec![
        cb("➕ Купить ключ", "buy"),
        cb("🎟 Промокод", "legacy:promo"),
    ]);
    rows.push(vec![cb("🔔 Уведомления", "guide:notifications")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn notification_settings_menu(expiry: bool, maintenance: bool) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            if expiry {
                "✅ Срок действия: включены"
            } else {
                "❌ Срок действия: выключены"
            },
            &format!("guide:notify-expiry-{}", if expiry { "off" } else { "on" }),
        )],
        vec![cb(
            if maintenance {
                "✅ Технические работы: включены"
            } else {
                "❌ Технические работы: выключены"
            },
            &format!(
                "guide:notify-maintenance-{}",
                if maintenance { "off" } else { "on" }
            ),
        )],
        vec![cb("⬅️ Профиль", "profile")],
    ])
}

pub fn portal_link_menu(url: &str) -> InlineKeyboardMarkup {
    let button = reqwest::Url::parse(url)
        .map(|url| InlineKeyboardButton::url("🚀 Войти в кабинет", url))
        .unwrap_or_else(|_| cb("Вернуться", "profile"));
    InlineKeyboardMarkup::new(vec![vec![button], vec![cb("⬅️ Кабинет", "profile")]])
}

pub fn customer_keyboard() -> KeyboardMarkup {
    let mut rows = vec![
        vec![
            KeyboardButton::new("🏠 Кабинет"),
            KeyboardButton::new("🔑 Мои ключи"),
        ],
        vec![
            KeyboardButton::new("➕ Купить ключ"),
            KeyboardButton::new("➕ Пополнить"),
        ],
        vec![
            KeyboardButton::new("📖 Инструкция"),
            KeyboardButton::new("🆘 Поддержка"),
        ],
        vec![KeyboardButton::new("🎟 Промокод")],
    ];
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs() as i64)
        .unwrap_or(0);
    if crate::calendar::legacy_requests_open(now) {
        rows.push(vec![KeyboardButton::new("♻️ Восстановить ключи")]);
    }
    KeyboardMarkup::new(rows).resize_keyboard().persistent()
}

pub fn legacy_restore_menu(eligible: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if eligible {
        rows.push(vec![cb("➕ Подать заявку на ключ", "legacy:request:new")]);
    } else {
        rows.push(vec![cb("🎟 Активировать промокод", "legacy:promo")]);
    }
    rows.push(vec![cb("⬅️ Кабинет", "profile")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn legacy_request_admin_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("✅ Одобрить и создать", &format!("legacy:req:ok:{id}")),
            cb("❌ Отклонить", &format!("legacy:req:no:{id}")),
        ],
        vec![cb("♻️ Все заявки", "admin:legacy")],
    ])
}

pub fn legacy_admin_menu(requests: &[crate::store::LegacyRequest]) -> InlineKeyboardMarkup {
    let mut rows = requests
        .iter()
        .flat_map(|r| {
            vec![
                vec![cb(
                    &format!("#{} · user {} · {}", r.id, r.user_id, r.requested_name),
                    &format!("admin:user:{}", r.user_id),
                )],
                vec![
                    cb("✅ Одобрить", &format!("legacy:req:ok:{}", r.id)),
                    cb("❌ Отклонить", &format!("legacy:req:no:{}", r.id)),
                ],
            ]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("💰 Изменить цену продления", "legacy:price")]);
    rows.push(vec![cb("🎟 Legacy-промокоды", "admin:promos")]);
    rows.push(vec![cb("⬅️ Админ-панель", "admin:dashboard")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn admin_keyboard() -> KeyboardMarkup {
    KeyboardMarkup::new(vec![
        vec![
            KeyboardButton::new("🏠 Админ-панель"),
            KeyboardButton::new("🔎 Поиск"),
        ],
        vec![
            KeyboardButton::new("🖥 Серверы"),
            KeyboardButton::new("🔑 Ключи"),
        ],
        vec![
            KeyboardButton::new("👥 Пользователи"),
            KeyboardButton::new("💳 Финансы"),
        ],
        vec![
            KeyboardButton::new("📊 Аналитика"),
            KeyboardButton::new("💬 Связь"),
        ],
        vec![
            KeyboardButton::new("🔄 Операции"),
            KeyboardButton::new("⚙️ Система"),
        ],
        vec![KeyboardButton::new("👤 Кабинет")],
    ])
    .resize_keyboard()
    .persistent()
}

pub fn admin_dashboard_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("🖥 Серверы", "admin:servers"),
            cb("🔑 Ключи", "admin:keys"),
        ],
        vec![
            cb("👥 Пользователи", "admin:users"),
            cb("💳 Финансы", "admin:finance"),
        ],
        vec![cb("🏷 Цены и промокоды", "admin:commerce")],
        vec![
            cb("📊 Аналитика", "stats"),
            cb("💬 Связь", "admin:communication"),
        ],
        vec![cb("🔄 Операции", "admin:operations")],
        vec![
            cb("⚙️ Система", "admin:system"),
            cb("🌐 Веб-кабинет", "portal"),
        ],
        vec![
            cb("🔎 Поиск", "admin:search"),
            cb("🔄 Обновить", "admin:dashboard"),
        ],
    ])
}

pub fn admin_operations_menu(server_ids: &[i64], has_unread: bool) -> InlineKeyboardMarkup {
    let mut rows = server_ids
        .iter()
        .take(8)
        .map(|id| {
            vec![cb(
                &format!("🖥 Открыть сервер #{id}"),
                &format!("server:{id}"),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("🔄 Проверить сейчас", "admin:operations:refresh")]);
    if has_unread {
        rows.push(vec![cb(
            "✅ Отметить события просмотренными",
            "admin:operations:ack",
        )]);
    }
    rows.push(vec![cb("⬅️ Админ-панель", "admin:dashboard")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn admin_keys_hub() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("➕ Создание ключей", "admin:create"),
            cb("📋 Все ключи", "list"),
        ],
        vec![cb("🗂 Группы", "groups"), cb("🔗 Владельцы", "admin:owners")],
        vec![cb("🧰 Массовые операции", "admin:bulk:menu")],
        vec![cb("🩺 Здоровье ключей", "admin:keys:health")],
        vec![cb("♻️ Восстановление старых", "admin:legacy")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn admin_key_health_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🔄 Обновить", "admin:keys:health")],
        vec![
            cb("🔴 Неактивные", "listfilter:offline"),
            cb("🟡 Не подключались", "listfilter:never"),
        ],
        vec![cb("📋 Все ключи", "list")],
        vec![cb("⬅️ К ключам", "admin:keys")],
    ])
}

pub fn admin_users_hub() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("📋 Все пользователи", "admin:owners"),
            cb("🔎 Поиск", "admin:search"),
        ],
        vec![cb("🧑‍💼 Роли сотрудников", "admin:roles")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn admin_communication_hub() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("🆘 Поддержка", "admin:support"),
            cb("📣 Рассылка", "admin:broadcast"),
        ],
        vec![cb("📝 Шаблоны рассылок", "admin:broadcast:templates")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn broadcast_templates_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("📣 Создать рассылку", "admin:broadcast")],
        vec![cb("⬅️ Связь", "admin:communication")],
    ])
}

pub fn broadcast_report_menu(id: i64, can_retry: bool) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if can_retry {
        rows.push(vec![cb(
            "🔁 Повторить только ошибки",
            &format!("broadcast:retry:{id}"),
        )]);
    }
    rows.push(vec![cb("⬅️ Связь", "admin:communication")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn admin_system_hub() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("⚙️ Настройки", "settings"),
            cb("💾 Резервные копии", "backup"),
        ],
        vec![
            cb("🗄 Копия БД сейчас", "admin:db-backup"),
            cb("🔍 Проверить архив БД", "admin:db-backup-audit"),
        ],
        vec![
            cb("🛡 VPN-служба", "admin:vpn"),
            cb("ℹ️ Справка", "admin:help"),
        ],
        vec![cb("⬆️ Обновить бота", "admin:update")],
        vec![cb(
            "🧪 Проверить систему сейчас",
            "admin:operations:refresh",
        )],
        vec![cb("📋 Журнал обновления", "admin:update:status")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn bot_update_confirm_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("⬆️ Установить обновление", "admin:update:run")],
        vec![cb("❌ Отмена", "admin:system")],
    ])
}

pub fn bot_update_status_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🔄 Обновить журнал", "admin:update:status")],
        vec![cb("↩️ Откатить бинарник", "admin:update:rollback")],
        vec![cb("⬅️ Система", "admin:system")],
    ])
}

pub fn servers_menu(servers: &[crate::store::VpnServer]) -> InlineKeyboardMarkup {
    let mut rows = servers
        .iter()
        .map(|server| {
            let icon = match server.status.as_str() {
                "online" => "🟢",
                "warning" => "🟠",
                "offline" => "🔴",
                "maintenance" => "🚧",
                _ => "⚪",
            };
            vec![cb(
                &format!(
                    "{icon} {}{} · {}",
                    if server.is_local { "🏠 " } else { "" },
                    server.name,
                    server.location
                ),
                &format!("server:{}", server.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![
        cb("➕ Подключить новый сервер", "server:add"),
        cb("💳 Календарь оплаты", "server:billing"),
    ]);
    rows.push(vec![cb("⬅️ Админ-панель", "admin:dashboard")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn server_setup_method_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "🚀 Установить AWG 1.0 автоматически",
            &format!("server:deploy:{id}"),
        )],
        vec![cb(
            "🔐 Подключить существующую панель",
            &format!("server:panel:{id}"),
        )],
        vec![cb(
            "🔗 Подключить bootstrap-командой",
            &format!("server:enroll:{id}"),
        )],
        vec![cb("⏭ Настроить позже", &format!("server:{id}"))],
    ])
}

pub fn server_card_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("— КЛЮЧИ —", &format!("server:{id}"))],
        vec![cb(
            "🎯 Новые ключи и замена",
            &format!("server:default:{id}"),
        )],
        vec![cb(
            "🔄 Синхронизировать панель",
            &format!("server:panel:sync:{id}"),
        )],
        vec![cb(
            "🧾 Сверить ключи с базой",
            &format!("server:panel:audit:{id}"),
        )],
        vec![cb(
            "📣 Владельцам ключей",
            &format!("broadcast:audience:server:{id}"),
        )],
        vec![cb("— СОСТОЯНИЕ —", &format!("server:{id}"))],
        vec![
            cb("🩺 Проверить ключи", &format!("server:check:{id}")),
            cb("🔬 Диагностика", &format!("server:diagnose:{id}")),
        ],
        vec![cb(
            "🧬 Проверить API панели",
            &format!("server:diagnose:{id}"),
        )],
        vec![
            cb(
                "🚧 Начать обслуживание",
                &format!("server:maintenance:{id}"),
            ),
            cb(
                "✅ Завершить обслуживание",
                &format!("server:maintenance:finish:{id}"),
            ),
        ],
        vec![cb(
            "🧪 Тестовая выдача ключа",
            &format!("server:probe:{id}"),
        )],
        vec![cb("— НАСТРОЙКА VPS —", &format!("server:{id}"))],
        vec![
            cb("✏️ Паспорт", &format!("server:edit:{id}")),
            cb("💳 Оплата", &format!("server:bill:{id}")),
        ],
        vec![
            cb("🔗 Подключить VPS", &format!("server:enroll:{id}")),
            cb(
                "🚫 Отозвать подключение",
                &format!("server:enroll:revoke:{id}"),
            ),
        ],
        vec![cb(
            "🚀 Установить VPN на VPS",
            &format!("server:deploy:{id}"),
        )],
        vec![cb(
            "🔐 Подключить / сменить панель",
            &format!("server:panel:{id}"),
        )],
        vec![cb(
            "🔀 Перевести AWG 2.0 → 1.0",
            &format!("server:migrate:{id}"),
        )],
        vec![cb("🛡 Управление VPN", "admin:vpn")],
        vec![
            cb("⬅️ Все серверы", "admin:servers"),
            cb("🏠 Админ-панель", "admin:dashboard"),
        ],
    ])
}

pub fn server_inventory_menu(
    id: i64,
    panel_only: bool,
    database_only: bool,
    wrong_server: bool,
) -> InlineKeyboardMarkup {
    let mut rows = Vec::new();
    if panel_only {
        rows.push(vec![cb(
            "📥 Импортировать ключи панели",
            &format!("server:panel:sync:{id}"),
        )]);
    }
    if database_only {
        rows.push(vec![cb(
            "🗄 Архивировать отсутствующие",
            &format!("server:panel:archive:{id}"),
        )]);
    }
    if wrong_server {
        rows.push(vec![cb(
            "📍 Исправить привязку",
            &format!("server:panel:rebind:{id}"),
        )]);
    }
    rows.push(vec![cb(
        "🔄 Проверить ещё раз",
        &format!("server:panel:audit:{id}"),
    )]);
    rows.push(vec![cb("⬅️ К серверу", &format!("server:{id}"))]);
    InlineKeyboardMarkup::new(rows)
}

pub fn server_inventory_confirm_menu(id: i64, operation: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "✅ Подтвердить",
            &format!("server:panel:{operation}:confirm:{id}"),
        )],
        vec![cb("↩️ Отмена", &format!("server:panel:audit:{id}"))],
    ])
}

pub fn server_maintenance_confirm_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "📣 Уведомить владельцев и начать",
            &format!("server:maintenance:notify:{id}"),
        )],
        vec![cb(
            "🔕 Начать без рассылки",
            &format!("server:maintenance:start:{id}"),
        )],
        vec![cb("❌ Отмена", &format!("server:{id}"))],
    ])
}

pub fn remote_migration_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "🧪 Предварительная проверка",
            &format!("server:migrate:preflight:{id}"),
        )],
        vec![cb("📍 Статус", &format!("server:migrate:status:{id}"))],
        vec![cb(
            "🧪 Получить тестовый конфиг",
            &format!("server:migrate:test:{id}"),
        )],
        vec![cb(
            "✅ Тест работает — включить",
            &format!("server:migrate:approve:{id}"),
        )],
        vec![cb(
            "🚨 Начать миграцию",
            &format!("server:migrate:ask:{id}"),
        )],
        vec![cb(
            "↩️ Аварийный откат",
            &format!("server:migrate:rollback:{id}"),
        )],
        vec![cb("⬅️ Карточка сервера", &format!("server:{id}"))],
    ])
}

pub fn remote_migration_confirm_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🚨 ДА, ЗАПУСТИТЬ", &format!("server:migrate:run:{id}"))],
        vec![cb("❌ Отмена", &format!("server:migrate:{id}"))],
    ])
}

pub fn vpn_service_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🩺 Проверить состояние", "check")],
        vec![cb("🔬 Подробная диагностика", "diagnose")],
        vec![cb("🔁 Перезапустить VPN", "restart")],
        vec![cb("🧰 Восстановить модуль", "repair")],
        vec![cb(
            "🔀 Переход локального сервера на AWG 1.0",
            "migration:local",
        )],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn local_migration_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🧪 Предварительная проверка", "migration:preflight")],
        vec![cb("🚨 Начать миграцию", "migration:start")],
        vec![cb("📍 Статус миграции", "migration:status")],
        vec![cb("↩️ Аварийный откат к AWG 2.0", "migration:rollback")],
        vec![cb("⬅️ Управление VPN", "admin:vpn")],
    ])
}

pub fn admin_create_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("➕ Создать один", "add"),
            cb("📦 Создать оптом", "addbulk"),
        ],
        vec![
            cb("👤 Пользователи", "admin:owners"),
            cb("🗂 Группы", "groups"),
        ],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn admin_roles_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("➕ Добавить сотрудника", "admin:role:add")],
        vec![cb("➖ Убрать сотрудника", "admin:role:remove")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn admin_promos_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🎟 Создать скидочный", "admin:promo:discount")],
        vec![cb("♻️ Создать Legacy", "admin:promo:legacy")],
        vec![cb("♻️ Legacy-заявки", "admin:legacy")],
        vec![cb("⬅️ Цены и промокоды", "admin:commerce")],
    ])
}

pub fn admin_commerce_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("₽ Тарифы", "admin:prices:rub"),
            cb("⭐ Stars", "admin:prices:stars"),
        ],
        vec![
            cb("🎟 Промокоды", "admin:promos"),
            cb("🤝 Реферальный %", "admin:referral"),
        ],
        vec![
            cb("♻️ Legacy-тариф", "legacy:price"),
            cb("💳 Реквизиты", "set:payment"),
        ],
        vec![cb("🏦 Эквайринг / Т-Банк", "set:acquiring")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn bulk_manage_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("⏸ Отключить по префиксу", "admin:bulk:disable")],
        vec![cb("▶️ Включить по префиксу", "admin:bulk:enable")],
        vec![cb("📅 Продлить по префиксу", "admin:bulk:extend")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn bulk_confirm_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("✅ Подтвердить операцию", "admin:bulk:confirm")],
        vec![cb("❌ Отмена", "admin:bulk:menu")],
    ])
}

pub fn statistics_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("📊 Общая сводка", "stats")],
        vec![
            cb("📶 Подключения", "stats:traffic"),
            cb("🖥 Серверы", "stats:servers"),
        ],
        vec![
            cb("👤 Пользователи", "stats:users"),
            cb("💳 Подписки", "stats:subscriptions"),
        ],
        vec![
            cb("📈 Тарифы", "stats:tariffs"),
            cb("💰 Доходы", "stats:finance"),
        ],
        vec![cb("🔑 Открыть ключи", "list"), cb("🗂 Группы", "groups")],
    ])
}

pub fn admin_users_menu(
    users: &[crate::store::UserRow],
    page: usize,
    pages: usize,
) -> InlineKeyboardMarkup {
    let mut rows = users
        .iter()
        .map(|u| {
            let label = u
                .username
                .as_ref()
                .map(|v| format!("@{v}"))
                .unwrap_or_else(|| format!("{} · {}", u.display_name, u.user_id));
            vec![cb(&label, &format!("admin:user:{}", u.user_id))]
        })
        .collect::<Vec<_>>();
    if pages > 1 {
        let mut navigation = Vec::new();
        if page > 0 {
            navigation.push(cb("⬅️", &format!("admin:owners:page:{}", page - 1)));
        }
        navigation.push(cb(&format!("{}/{}", page + 1, pages), "noop"));
        if page + 1 < pages {
            navigation.push(cb("➡️", &format!("admin:owners:page:{}", page + 1)));
        }
        rows.push(navigation);
    }
    rows.push(vec![cb("🔎 Найти пользователя", "admin:search")]);
    rows.push(vec![cb("⬅️ Админ-панель", "admin:dashboard")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn admin_user_menu(user_id: i64, blocked: bool) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("🔑 Ключи", &format!("admin:userkeys:{user_id}")),
            cb("💳 Платежи", &format!("admin:userpay:{user_id}")),
        ],
        vec![
            cb("➕ Баланс", &format!("admin:userbal:{user_id}")),
            cb("📝 Заметка", &format!("admin:usernote:{user_id}")),
        ],
        vec![cb(
            "🏷 Индивидуальная скидка",
            &format!("admin:userdiscount:{user_id}"),
        )],
        vec![cb(
            if blocked {
                "✅ Разблокировать"
            } else {
                "⛔ Заблокировать"
            },
            &format!(
                "admin:userblock:{user_id}:{}",
                if blocked { "off" } else { "on" }
            ),
        )],
        vec![cb("⬅️ Пользователи", "admin:owners")],
    ])
}

pub fn admin_user_keys_menu(user_id: i64, names: &[String]) -> InlineKeyboardMarkup {
    let mut rows = names
        .iter()
        .flat_map(|name| {
            vec![
                vec![cb(&format!("🔑 {name}"), &format!("show:{name}"))],
                vec![
                    cb("👁 Открыть", &format!("show:{name}")),
                    cb("🗑 Удалить", &format!("del:{name}")),
                ],
            ]
        })
        .collect::<Vec<_>>();
    if !names.is_empty() {
        rows.push(vec![cb(
            "🗑 Удалить все ключи",
            &format!("admin:userkeys-delete:{user_id}"),
        )]);
    }
    rows.push(vec![cb(
        "⬅️ Карточка пользователя",
        &format!("admin:user:{user_id}"),
    )]);
    InlineKeyboardMarkup::new(rows)
}

pub fn admin_user_delete_keys_confirm_menu(user_id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "🚨 Да, удалить все ключи",
            &format!("admin:userkeys-delete-confirm:{user_id}"),
        )],
        vec![cb("Отмена", &format!("admin:userkeys:{user_id}"))],
    ])
}

pub fn buy_terms_menu(prices: [i64; 4]) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb(&format!("1 месяц — {} ₽", prices[0] / 100), "buy:term:1"),
            cb(&format!("3 месяца — {} ₽", prices[1] / 100), "buy:term:3"),
        ],
        vec![
            cb(&format!("6 месяцев — {} ₽", prices[2] / 100), "buy:term:6"),
            cb(
                &format!("12 месяцев — {} ₽", prices[3] / 100),
                "buy:term:12",
            ),
        ],
    ])
}

pub fn buy_servers_menu(
    servers: &[crate::store::VpnServer],
    store: &crate::store::Store,
) -> InlineKeyboardMarkup {
    let mut rows = servers
        .iter()
        .map(|server| {
            let protocol = if server.protocol == "amneziawg-2" {
                "AWG 2.0 · тестовый"
            } else {
                "AWG 1.0 · стабильно"
            };
            let used = store.server_client_count(server.id);
            let free = server.capacity.saturating_sub(used);
            vec![cb(
                &format!("📍 {} · {protocol} · свободно {free}", server.location),
                &format!("buy:server:{}", server.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("⬅️ Кабинет", "profile")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn buy_balance_confirm_menu(months: i64, nonce: u64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "✅ Списать баланс и создать ключ",
            &format!("buy:method:{months}:balance-go-{nonce}"),
        )],
        vec![cb("Отмена", "buy")],
    ])
}

pub fn renew_balance_confirm_menu(
    name: &str,
    months: i64,
    expected_expiry: i64,
) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "✅ Списать баланс и продлить",
            &format!("renew:method:{name}:{months}:balance-go-{expected_expiry}"),
        )],
        vec![cb("Отмена", &format!("mykey:{name}"))],
    ])
}

pub fn buy_method_menu(months: i64, acquiring: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![cb("💳 Перевод", &format!("buy:method:{months}:manual"))],
        vec![cb(
            "💰 Внутренний баланс",
            &format!("buy:method:{months}:balance"),
        )],
        vec![cb(
            "⭐ Telegram Stars",
            &format!("buy:method:{months}:stars"),
        )],
    ];
    if acquiring {
        rows.insert(
            0,
            vec![cb(
                "🏦 Оплатить онлайн",
                &format!("buy:method:{months}:acquiring"),
            )],
        );
    }
    InlineKeyboardMarkup::new(rows)
}

pub fn bulk_servers_menu(servers: &[crate::store::VpnServer]) -> InlineKeyboardMarkup {
    let mut rows = servers
        .iter()
        .map(|server| {
            vec![cb(
                &format!(
                    "📍 {} · {}",
                    server.location,
                    if server.protocol == "amneziawg-2" {
                        "AWG 2.0"
                    } else {
                        "AWG 1.0"
                    }
                ),
                &format!("bulkserver:{}", server.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("❌ Отмена", "admin:keys")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn payment_paid_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![cb("✅ Я оплатил", &format!("buy:paid:{id}"))]])
}

pub fn payment_admin_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        cb("✅ Одобрить", &format!("pay:ok:{id}")),
        cb("❌ Отклонить", &format!("pay:no:{id}")),
    ]])
}

pub fn support_tickets_menu(tickets: &[crate::store::SupportTicket]) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(tickets.iter().map(|t| {
        vec![cb(
            &format!("#{} · {} · {} · {}", t.id, t.category, t.priority, t.status),
            &format!("support:ticket:{}", t.id),
        )]
    }))
}

pub fn support_category_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("🔌 Подключение", "support:new:connection"),
            cb("💳 Оплата", "support:new:payment"),
        ],
        vec![
            cb("🐞 Ошибка", "support:new:bug"),
            cb("💬 Другой вопрос", "support:new:general"),
        ],
    ])
}

pub fn support_ticket_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🙋 Взять в работу", &format!("support:take:{id}"))],
        vec![
            cb("✉️ Ответить", &format!("support:reply:{id}")),
            cb("✅ Закрыть", &format!("support:close:{id}")),
        ],
        vec![
            cb("Обычный", &format!("support:priority:{id}:normal")),
            cb("Высокий", &format!("support:priority:{id}:high")),
            cb("Срочный", &format!("support:priority:{id}:urgent")),
        ],
        vec![cb("⬅️ Обращения", "admin:support")],
    ])
}

pub fn support_rating_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![(1..=5)
        .map(|v| cb(&format!("{v}⭐"), &format!("support:rate:{id}:{v}")))
        .collect::<Vec<_>>()])
}

pub fn support_filters_menu(tickets: &[crate::store::SupportTicket]) -> InlineKeyboardMarkup {
    let mut rows = vec![vec![
        cb("🆕 Новые", "support:filter:open"),
        cb("🛠 В работе", "support:filter:in_progress"),
        cb("✅ Закрытые", "support:filter:closed"),
    ]];
    rows.extend(tickets.iter().map(|t| {
        vec![cb(
            &format!("#{} · {} · {} · {}", t.id, t.category, t.priority, t.status),
            &format!("support:ticket:{}", t.id),
        )]
    }));
    rows.push(vec![cb("⬅️ Админ-панель", "admin:dashboard")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn finance_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("📥 Скачать CSV", "finance:export")],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn finance_dashboard_menu(payments: &[crate::store::PaymentRequest]) -> InlineKeyboardMarkup {
    let mut rows = payments
        .iter()
        .take(20)
        .flat_map(|p| {
            vec![
                vec![cb(
                    &format!(
                        "#{} · user {} · {:.2} ₽",
                        p.id,
                        p.user_id,
                        p.amount_kopecks as f64 / 100.0
                    ),
                    &format!("admin:userpay:{}", p.user_id),
                )],
                vec![
                    cb("✅ Одобрить", &format!("pay:ok:{}", p.id)),
                    cb("❌ Отклонить", &format!("pay:no:{}", p.id)),
                ],
            ]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("📥 Скачать CSV", "finance:export")]);
    rows.push(vec![cb("⬅️ Админ-панель", "admin:dashboard")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn broadcast_audience_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("👥 Все", "broadcast:audience:all"),
            cb("✅ С активными ключами", "broadcast:audience:active"),
        ],
        vec![
            cb("⏳ Истекают за 7 дней", "broadcast:audience:expiring"),
            cb("🆕 Без ключей", "broadcast:audience:nokeys"),
        ],
        vec![cb("⬅️ Админ-панель", "admin:dashboard")],
    ])
}

pub fn customer_keys_menu(items: &[(String, String)]) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(
        items
            .iter()
            .map(|(name, title)| vec![cb(title, &format!("mykey:{name}"))]),
    )
}

pub fn customer_key_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "📲 Получить конфигурацию и QR",
            &format!("refresh:{name}"),
        )],
        vec![cb("📖 Как установить", &format!("guide:install:{name}"))],
        vec![cb("🩺 Не подключается", &format!("guide:trouble:{name}"))],
        vec![
            cb("📅 Продлить", &format!("renew:{name}")),
            cb("✏️ Устройство", &format!("device:label:{name}")),
        ],
        vec![cb(
            "🛟 Замена при недоступном сервере",
            &format!("move:choose:{name}"),
        )],
        vec![cb("⬅️ Мои ключи", "mykeys")],
    ])
}

pub fn expired_subscription_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("🚀 ВОЗОБНОВИТЬ ТАРИФ", &format!("renew:{name}"))],
        vec![cb("🆘 Нужна помощь", "support:new:connection")],
        vec![cb("⬅️ Мои ключи", "mykeys")],
    ])
}

pub fn instructions_menu() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("📱 AmneziaVPN", "guide:amnezia")],
        vec![cb("🛡 AmneziaWG", "guide:awg")],
        vec![cb("🩺 Не подключается", "guide:trouble")],
        vec![cb("🆘 Поддержка", "support:new:connection")],
    ])
}

pub fn installation_platform_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("🤖 Android", &format!("guide:android:{name}")),
            cb("🍎 iPhone/iPad", &format!("guide:ios:{name}")),
        ],
        vec![
            cb("🪟 Windows", &format!("guide:windows:{name}")),
            cb("💻 macOS", &format!("guide:macos:{name}")),
        ],
        vec![cb("⬅️ К ключу", &format!("mykey:{name}"))],
    ])
}

pub fn troubleshooting_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "1️⃣ Проверить сеть",
            &format!("guide:check-network:{name}"),
        )],
        vec![cb(
            "2️⃣ Проверить приложение",
            &format!("guide:check-app:{name}"),
        )],
        vec![cb("3️⃣ Получить свежий конфиг", &format!("refresh:{name}"))],
        vec![cb(
            "🆘 Отправить диагностику",
            &format!("support:diagnostic:{name}"),
        )],
        vec![cb("⬅️ К ключу", &format!("mykey:{name}"))],
    ])
}

pub fn customer_move_servers_menu(
    name: &str,
    servers: &[crate::store::VpnServer],
    store: &crate::store::Store,
) -> InlineKeyboardMarkup {
    let mut rows = servers
        .iter()
        .map(|server| {
            vec![cb(
                &format!(
                    "📍 {} · {} ({}/{})",
                    server.location,
                    server.protocol,
                    store.server_client_count(server.id),
                    server.capacity
                ),
                &format!("move:run:{name}:{}", server.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("⬅️ Назад", &format!("mykey:{name}"))]);
    InlineKeyboardMarkup::new(rows)
}

pub fn customer_refresh_confirm_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "✅ Создать свежую конфигурацию",
            &format!("refreshgo:{name}"),
        )],
        vec![cb("⬅️ Назад к ключу", &format!("mykey:{name}"))],
    ])
}

pub fn replacement_confirm_menu(id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb("✅ Новый ключ работает", &format!("move:confirm:{id}"))],
        vec![cb("❌ Не подключается", &format!("move:cancel:{id}"))],
        vec![cb("📖 Как установить", "guide:awg")],
    ])
}

pub fn renew_terms_menu(name: &str, prices: [i64; 4]) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb(
                &format!("1 месяц — {} ₽", prices[0] / 100),
                &format!("renew:term:{name}:1"),
            ),
            cb(
                &format!("3 месяца — {} ₽", prices[1] / 100),
                &format!("renew:term:{name}:3"),
            ),
        ],
        vec![
            cb(
                &format!("6 месяцев — {} ₽", prices[2] / 100),
                &format!("renew:term:{name}:6"),
            ),
            cb(
                &format!("12 месяцев — {} ₽", prices[3] / 100),
                &format!("renew:term:{name}:12"),
            ),
        ],
    ])
}

pub fn auto_renew_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("✅ Авто: 1 месяц", &format!("autorenew:{name}:1:on")),
            cb("✅ Авто: 3 месяца", &format!("autorenew:{name}:3:on")),
        ],
        vec![
            cb("✅ Авто: 6 месяцев", &format!("autorenew:{name}:6:on")),
            cb("✅ Авто: 12 месяцев", &format!("autorenew:{name}:12:on")),
        ],
        vec![cb(
            "❌ Выключить автопродление",
            &format!("autorenew:{name}:1:off"),
        )],
    ])
}

pub fn renew_method_menu(name: &str, months: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "💳 Перевод",
            &format!("renew:method:{name}:{months}:manual"),
        )],
        vec![cb(
            "💰 Внутренний баланс",
            &format!("renew:method:{name}:{months}:balance"),
        )],
        vec![cb(
            "⭐ Telegram Stars",
            &format!("renew:method:{name}:{months}:stars"),
        )],
    ])
}

pub fn legacy_renew_menu(name: &str, price_kopecks: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            &format!("💳 Продлить за {:.2} ₽", price_kopecks as f64 / 100.0),
            &format!("legacy:renew:{name}"),
        )],
        vec![cb("⬅️ Мои ключи", "mykeys")],
    ])
}

pub fn legacy_renew_method_menu(name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            "💳 Перевод",
            &format!("legacy:renew:method:{name}:manual"),
        )],
        vec![cb(
            "💰 Внутренний баланс",
            &format!("legacy:renew:method:{name}:balance"),
        )],
        vec![cb("⬅️ Назад", &format!("legacy:renew:{name}"))],
    ])
}

pub fn main_menu(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(&i18n::btn_clients(lang), "list")],
        vec![cb(&i18n::btn_add(lang), "add")],
        vec![cb(&i18n::btn_bulk(lang), "addbulk")],
        vec![cb(&i18n::btn_stats(lang), "stats")],
        vec![cb(&i18n::btn_backup(lang), "backup")],
        vec![
            cb(&i18n::btn_check(lang), "check"),
            cb(&i18n::btn_diagnose(lang), "diagnose"),
        ],
        vec![
            cb(&i18n::btn_restart(lang), "restart"),
            cb(&i18n::btn_repair(lang), "repair"),
        ],
        vec![cb(&i18n::btn_groups(lang), "groups")],
        vec![cb(&i18n::btn_settings(lang), "settings")],
    ])
}

/// Раздел «Группы» — только для владельцев (handlers не показывают его другим).
pub fn groups_menu(lang: Lang, groups: &[(crate::store::GroupRow, i64)]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = groups
        .iter()
        .map(|(g, n)| {
            vec![cb(
                &format!("🗂 {} ({n})", g.name),
                &format!("g:card:{}", g.id),
            )]
        })
        .collect();
    rows.push(vec![cb(&i18n::btn_group_create(lang), "g:new")]);
    rows.push(vec![cb(&i18n::btn_back(lang), "menu")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn group_card_menu(lang: Lang, id: i64, has_invite: bool) -> InlineKeyboardMarkup {
    let invite_btn = if has_invite {
        cb(&i18n::btn_invite_revoke(lang), &format!("g:invrev:{id}"))
    } else {
        cb(&i18n::btn_group_invite(lang), &format!("g:inv:{id}"))
    };
    InlineKeyboardMarkup::new(vec![
        // «Клиенты группы» (#20) — существующий скоуп-фильтр списка: тот же
        // callback, что и на экране выбора группового фильтра (gscope:{id}),
        // устанавливает липкий фильтр владельца и рендерит список группы.
        vec![cb(&i18n::btn_group_clients(lang), &format!("gscope:{id}"))],
        vec![
            cb(&i18n::btn_group_rename(lang), &format!("g:ren:{id}")),
            cb(&i18n::btn_group_quota(lang), &format!("g:quota:{id}")),
        ],
        vec![cb(&i18n::btn_group_admins(lang), &format!("g:adm:{id}"))],
        vec![
            invite_btn,
            cb(&i18n::btn_admin_by_id(lang), &format!("g:admid:{id}")),
        ],
        vec![cb(&i18n::btn_group_regen(lang), &format!("g:regen:{id}"))],
        vec![cb(&i18n::btn_group_delete(lang), &format!("g:del:{id}"))],
        vec![cb(&i18n::btn_back(lang), "groups")],
    ])
}

pub fn group_admins_menu(lang: Lang, id: i64, admins: &[i64]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = admins
        .iter()
        .map(|uid| vec![cb(&format!("❌ {uid}"), &format!("g:admdel:{id}:{uid}"))])
        .collect();
    rows.push(vec![cb(&i18n::btn_back(lang), &format!("g:card:{id}"))]);
    InlineKeyboardMarkup::new(rows)
}

pub fn group_delete_choice_menu(lang: Lang, id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            &i18n::btn_delete_detach(lang),
            &format!("g:deldetach:{id}"),
        )],
        vec![cb(
            &i18n::btn_delete_with_clients(lang),
            &format!("g:delall:{id}"),
        )],
        vec![cb(&i18n::btn_back(lang), &format!("g:card:{id}"))],
    ])
}

pub fn confirm_group_delete_clients_menu(lang: Lang, id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(
            &i18n::btn_delete_with_clients(lang),
            &format!("g:delallyes:{id}"),
        )],
        vec![cb(&i18n::btn_back(lang), &format!("g:card:{id}"))],
    ])
}

pub fn confirm_group_regen_menu(lang: Lang, id: i64) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(&i18n::btn_group_regen(lang), &format!("g:regengo:{id}"))],
        vec![cb(&i18n::btn_back(lang), &format!("g:card:{id}"))],
    ])
}

pub fn group_select_menu(lang: Lang, groups: &[crate::store::GroupRow]) -> InlineKeyboardMarkup {
    let rows: Vec<Vec<InlineKeyboardButton>> = groups
        .iter()
        .map(|g| vec![cb(&format!("🗂 {}", g.name), &format!("g:sel:{}", g.id))])
        .collect();
    let _ = lang; // подписи — имена групп, локализация не нужна
    InlineKeyboardMarkup::new(rows)
}

/// Меню группового админа: список/добавить/статистика группы; смена группы —
/// только если групп несколько.
pub fn ga_main_menu(lang: Lang, multi: bool) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![cb(&i18n::btn_clients(lang), "list")],
        vec![cb(&i18n::btn_add(lang), "add")],
        vec![cb(&i18n::btn_bulk(lang), "addbulk")],
        vec![cb(&i18n::btn_stats(lang), "stats")],
    ];
    if multi {
        rows.push(vec![cb(&i18n::btn_switch_group(lang), "g:selmenu")]);
    }
    InlineKeyboardMarkup::new(rows)
}

pub fn move_client_menu(
    lang: Lang,
    name: &str,
    groups: &[crate::store::GroupRow],
) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = groups
        .iter()
        .map(|g| {
            vec![cb(
                &format!("🗂 {}", g.name),
                &format!("gmoveto:{}:{name}", g.id),
            )]
        })
        .collect();
    rows.push(vec![cb(
        &i18n::no_group_label(lang),
        &format!("gmoveto:none:{name}"),
    )]);
    rows.push(vec![cb(&i18n::btn_back(lang), &format!("client:{name}"))]);
    InlineKeyboardMarkup::new(rows)
}

/// Экран выбора группового фильтра списка (владелец): все / без группы / группа.
pub fn group_scope_menu(lang: Lang, groups: &[crate::store::GroupRow]) -> InlineKeyboardMarkup {
    let mut rows = vec![
        vec![cb(&i18n::btn_scope_all(lang), "gscope:all")],
        vec![cb(&i18n::no_group_label(lang), "gscope:none")],
    ];
    rows.extend(
        groups
            .iter()
            .map(|g| vec![cb(&format!("🗂 {}", g.name), &format!("gscope:{}", g.id))]),
    );
    rows.push(vec![cb(&i18n::btn_back(lang), "list")]);
    InlineKeyboardMarkup::new(rows)
}

/// Экран выбора языка при первом запуске — показывает оба варианта
/// одновременно (ещё не знаем предпочтение пользователя), без опоры на `lang`.
pub fn language_select() -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        cb("🇷🇺 Русский", "lang:ru"),
        cb("🇬🇧 English", "lang:en"),
    ]])
}

pub fn settings_menu(
    lang: Lang,
    psk_default: bool,
    name_slug: bool,
    deliver_conf: bool,
    deliver_qr: bool,
    deliver_link: bool,
) -> InlineKeyboardMarkup {
    let _ = name_slug;
    InlineKeyboardMarkup::new(vec![
        vec![
            cb(&i18n::btn_lang_ru(lang), "set:lang:ru"),
            cb(&i18n::btn_lang_en(lang), "set:lang:en"),
        ],
        vec![cb(
            &i18n::btn_psk_toggle(lang, psk_default),
            if psk_default {
                "set:psk:off"
            } else {
                "set:psk:on"
            },
        )],
        vec![cb(
            &i18n::btn_conf_toggle(lang, deliver_conf),
            if deliver_conf {
                "set:conf:off"
            } else {
                "set:conf:on"
            },
        )],
        vec![cb(
            &i18n::btn_qr_toggle(lang, deliver_qr),
            if deliver_qr {
                "set:qr:off"
            } else {
                "set:qr:on"
            },
        )],
        vec![cb(
            &i18n::btn_link_toggle(lang, deliver_link),
            if deliver_link {
                "set:link:off"
            } else {
                "set:link:on"
            },
        )],
        vec![cb("💳 Реквизиты оплаты", "set:payment")],
        vec![cb(&i18n::btn_back(lang), "admin:dashboard")],
    ])
}

// Подписи пресетов срока действия не входят в каталог `i18n` (см. brief задачи
// 5) — локализуются здесь напрямую, без изменения `i18n.rs`.
fn day_label(lang: Lang, days: u32) -> String {
    match lang {
        Lang::Ru => format!("{days}д"),
        Lang::En => format!("{days}d"),
    }
}

pub fn expiry_menu(lang: Lang) -> InlineKeyboardMarkup {
    let none_txt = match lang {
        Lang::Ru => "Без срока",
        Lang::En => "No expiry",
    };
    let custom_txt = match lang {
        Lang::Ru => "✏️ Свой",
        Lang::En => "✏️ Custom",
    };
    InlineKeyboardMarkup::new(vec![
        vec![cb(none_txt, "exp:none")],
        vec![
            cb(&day_label(lang, 1), "exp:1d"),
            cb(&day_label(lang, 7), "exp:7d"),
            cb(&day_label(lang, 14), "exp:14d"),
        ],
        vec![
            cb(&day_label(lang, 30), "exp:30d"),
            cb(&day_label(lang, 90), "exp:90d"),
            cb(&day_label(lang, 180), "exp:180d"),
        ],
        vec![
            cb(&day_label(lang, 365), "exp:365d"),
            cb(custom_txt, "exp:custom"),
        ],
    ])
}

/// Экран выбора количества для массовой генерации. Callback `bulk:N`.
pub fn bulk_count_menu(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb("1", "bulk:1"),
            cb("3", "bulk:3"),
            cb("5", "bulk:5"),
            cb("10", "bulk:10"),
        ],
        vec![
            cb("20", "bulk:20"),
            cb("50", "bulk:50"),
            cb("99", "bulk:99"),
        ],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

/// Экран выбора срока для массовой генерации. Копия `expiry_menu`, но с
/// `bulkexp:` префиксами (чтобы bulk- и одиночный-потоки шли через разные
/// Action без условной логики в общем обработчике `Expiry`).
pub fn bulk_expiry_menu(lang: Lang) -> InlineKeyboardMarkup {
    let none_txt = match lang {
        Lang::Ru => "Без срока",
        Lang::En => "No expiry",
    };
    let custom_txt = match lang {
        Lang::Ru => "✏️ Свой",
        Lang::En => "✏️ Custom",
    };
    InlineKeyboardMarkup::new(vec![
        vec![cb(none_txt, "bulkexp:none")],
        vec![
            cb(&day_label(lang, 1), "bulkexp:1d"),
            cb(&day_label(lang, 7), "bulkexp:7d"),
            cb(&day_label(lang, 14), "bulkexp:14d"),
        ],
        vec![
            cb(&day_label(lang, 30), "bulkexp:30d"),
            cb(&day_label(lang, 90), "bulkexp:90d"),
            cb(&day_label(lang, 180), "bulkexp:180d"),
        ],
        vec![
            cb(&day_label(lang, 365), "bulkexp:365d"),
            cb(custom_txt, "bulkexp:custom"),
        ],
    ])
}

/// Шаг выбора PSK в диалоге `add` — дефолтная опция (по настройке
/// `settings.psk_default()`) идёт первой кнопкой.
pub fn psk_step(lang: Lang, default_on: bool) -> InlineKeyboardMarkup {
    let (first, second) = if default_on {
        (
            cb(&i18n::btn_create_with_psk(lang), "add:psk:on"),
            cb(&i18n::btn_create_no_psk(lang), "add:psk:off"),
        )
    } else {
        (
            cb(&i18n::btn_create_no_psk(lang), "add:psk:off"),
            cb(&i18n::btn_create_with_psk(lang), "add:psk:on"),
        )
    };
    InlineKeyboardMarkup::new(vec![
        vec![first, second],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

pub fn add_server_menu(servers: &[crate::store::VpnServer]) -> InlineKeyboardMarkup {
    let mut rows = servers
        .iter()
        .map(|server| {
            vec![cb(
                &format!("🌍 {} · {}", server.name, server.location),
                &format!("add:server:{}", server.id),
            )]
        })
        .collect::<Vec<_>>();
    rows.push(vec![cb("❌ Отмена", "menu")]);
    InlineKeyboardMarkup::new(rows)
}

/// Шаг выбора PSK в bulk-диалоге — как `psk_step`, но с `bulkadd:psk:` callback'ами
/// (чтобы попасть в Action::AddBulkPsk, а не в одиночный Action::AddPsk).
pub fn bulk_psk_step(lang: Lang, default_on: bool) -> InlineKeyboardMarkup {
    let (first, second) = if default_on {
        (
            cb(&i18n::btn_create_with_psk(lang), "bulkadd:psk:on"),
            cb(&i18n::btn_create_no_psk(lang), "bulkadd:psk:off"),
        )
    } else {
        (
            cb(&i18n::btn_create_no_psk(lang), "bulkadd:psk:off"),
            cb(&i18n::btn_create_with_psk(lang), "bulkadd:psk:on"),
        )
    };
    InlineKeyboardMarkup::new(vec![
        vec![first, second],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

/// Ряд фильтра списка: [Все] [🟢 Онлайн] [🔴 Оффлайн] [🟡 Никогда].
/// Активный фильтр помечается ✅-префиксом. Подписи локализуются здесь
/// (как day_label), не входят в каталог i18n. Callback `listfilter:{as_str}`.
fn filter_label(lang: Lang, f: ClientFilter) -> String {
    let mark = f.mark();
    let name = match (lang, f) {
        (Lang::Ru, ClientFilter::All) => "Все",
        (Lang::En, ClientFilter::All) => "All",
        (Lang::Ru, ClientFilter::Online) => "Подключены",
        (Lang::En, ClientFilter::Online) => "Online",
        (Lang::Ru, ClientFilter::Offline) => "Оффлайн",
        (Lang::En, ClientFilter::Offline) => "Offline",
        (Lang::Ru, ClientFilter::Never) => "Никогда",
        (Lang::En, ClientFilter::Never) => "Never",
    };
    format!("{mark} {name}")
}

fn filter_row(lang: Lang, current: ClientFilter) -> Vec<InlineKeyboardButton> {
    [
        ClientFilter::All,
        ClientFilter::Online,
        ClientFilter::Offline,
        ClientFilter::Never,
    ]
    .iter()
    .map(|&f| {
        // Активный фильтр помечается ✅ — кнопка-переключатель, неубираемый
        // индикатор текущего режима просмотра списка.
        let prefix = if f == current { "✅ " } else { "" };
        cb(
            &format!("{prefix}{}", filter_label(lang, f)),
            &format!("listfilter:{}", f.as_str()),
        )
    })
    .collect()
}

#[allow(clippy::too_many_arguments)]
pub fn clients_list(
    lang: Lang,
    clients: &[Client],
    expiries: &[Option<i64>],
    now: i64,
    page: usize,
    per_page: usize,
    current_filter: ClientFilter,
    is_owner: bool,
) -> InlineKeyboardMarkup {
    if per_page == 0 {
        return InlineKeyboardMarkup::new(vec![vec![cb(&i18n::btn_back(lang), "menu")]]);
    }

    let start = page * per_page;
    let mut rows: Vec<Vec<InlineKeyboardButton>> = clients
        .iter()
        .enumerate()
        .skip(start)
        .take(per_page)
        .map(|(i, c)| {
            let mark = c.mark(now);
            let state = crate::vpn::model::status_label(lang, c, now);
            // Компактный handshake («2 мин», «никогда») — требуется stats()
            // (last_handshake есть только в stats --json, не в list --json).
            let hs = format_handshake_compact(lang, now, c.last_handshake.unwrap_or(0));
            let exp = expiries.get(i).copied().flatten();
            let label = match crate::vpn::model::format_expiry_badge(lang, now, exp) {
                Some(badge) => format!("{mark} {} · {state} · {hs} {badge}", c.name),
                None => format!("{mark} {} · {state} · {hs}", c.name),
            };
            vec![cb(&label, &format!("client:{}", c.name))]
        })
        .collect();

    let total_pages = clients.len().div_ceil(per_page).max(1);
    // 🔄 всегда в nav-ряду: перерисовывает ТЕКУЩУЮ страницу со свежими данными.
    // Callback `page:{page}` → Action::Page (он заново зовёт vpn.list_enriched() —
    // список берёт status_code из list и last_handshake/rx/tx из stats), поэтому
    // refresh сохраняет страницу, а не сбрасывает на 0. На одностраничном списке
    // это единственная кнопка ряда; на многостраничном встаёт между пагинацией:
    // [◀️] [🔄] [▶️].
    let mut nav = Vec::new();
    if page > 0 {
        nav.push(cb("◀️", &format!("page:{}", page - 1)));
    }
    nav.push(cb(
        &format!("🔄 {}/{}", page + 1, total_pages),
        &format!("page:{page}"),
    ));
    if page + 1 < total_pages {
        nav.push(cb("▶️", &format!("page:{}", page + 1)));
    }
    rows.push(nav);
    let filter_btns = filter_row(lang, current_filter);
    // Кнопка «🗂» — фильтр списка по группе; видна только владельцу
    // (групповому админу скоуп и так задаёт его текущая группа).
    rows.push(filter_btns);
    if is_owner {
        let scope = match lang {
            Lang::Ru => "🗂 Фильтр по группе",
            Lang::En => "🗂 Group filter",
        };
        rows.push(vec![cb(scope, "gscope")]);
    }
    // «Перевыпустить всех» — глобальное действие, групповому админу не
    // показываем (Action::RegenAll всё равно owner-only).
    if is_owner {
        rows.push(vec![cb(&i18n::btn_regen_all(lang), "regen_all")]);
    }
    rows.push(vec![cb(&i18n::btn_back(lang), "menu")]);
    InlineKeyboardMarkup::new(rows)
}

/// Клавиатура пустой выборки списка: клиенты на сервере есть, но липкий
/// статус-фильтр/групповой скоуп ничего не пропустил. Обязана оставлять
/// кнопки смены фильтра и (владельцу) скоупа — иначе «Без группы» при
/// полностью распределённых клиентах запирает раздел клиентов (#20).
pub fn clients_empty_menu(
    lang: Lang,
    current_filter: ClientFilter,
    is_owner: bool,
) -> InlineKeyboardMarkup {
    let filter_btns = filter_row(lang, current_filter);
    let show_all = match lang {
        Lang::Ru => "👥 Показать все ключи",
        Lang::En => "👥 Show all keys",
    };
    let mut rows = vec![vec![cb(show_all, "listfilter:all")], filter_btns];
    if is_owner {
        let scope = match lang {
            Lang::Ru => "🗂 Фильтр по группе",
            Lang::En => "🗂 Group filter",
        };
        rows.push(vec![cb(scope, "gscope")]);
    }
    rows.push(vec![cb(&i18n::btn_back(lang), "menu")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn client_card(lang: Lang, name: &str, is_owner: bool) -> InlineKeyboardMarkup {
    let conf_txt = match lang {
        Lang::Ru => "📄 Конфиг",
        Lang::En => "📄 Config",
    };
    let del_txt = match lang {
        Lang::Ru => "🗑 Удалить",
        Lang::En => "🗑 Delete",
    };
    let mut rows = vec![
        vec![
            cb(conf_txt, &format!("conf:{name}")),
            cb(&i18n::btn_card_qr(lang), &format!("qr:{name}")),
        ],
        vec![
            cb(&i18n::btn_card_link(lang), &format!("uri:{name}")),
            cb(&i18n::btn_card_all(lang), &format!("all:{name}")),
        ],
        vec![
            cb(&i18n::btn_regen(lang), &format!("regen:{name}")),
            cb(del_txt, &format!("del:{name}")),
        ],
    ];
    // «Изменить» и «Перенести» — owner-only (их Action'ы гейтятся ролью,
    // групповому админу мёртвые кнопки не показываем); «История» — всем.
    let mut util_row = Vec::new();
    if is_owner {
        rows.push(vec![cb(
            "👤 Назначить владельца",
            &format!("owner:assign:{name}"),
        )]);
        rows.push(vec![cb(
            "📅 Изменить срок действия",
            &format!("owner:expiry:{name}"),
        )]);
        rows.push(vec![
            cb("⏸ Отключить", &format!("owner:disable:{name}")),
            cb("▶️ Включить", &format!("owner:enable:{name}")),
        ]);
        rows.push(vec![cb(
            "📝 Заметка о ключе",
            &format!("client:note:{name}"),
        )]);
        util_row.push(cb(&i18n::btn_modify(lang), &format!("mod:{name}")));
    }
    util_row.push(cb(&i18n::btn_history(lang), &format!("history:{name}")));
    rows.push(util_row);
    if is_owner {
        rows.push(vec![cb(
            &i18n::btn_client_move(lang),
            &format!("gmove:{name}"),
        )]);
    }
    rows.push(vec![cb(&i18n::btn_back(lang), "menu")]);
    InlineKeyboardMarkup::new(rows)
}

/// Клавиатура экрана «История»: одна кнопка возврата к карточке клиента
/// (не к главному меню — история открывается из карточки).
pub fn client_history(lang: Lang, name: &str) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![cb(
        &i18n::btn_history_back(lang),
        &format!("client:{name}"),
    )]])
}

pub fn confirm_delete(lang: Lang, name: &str) -> InlineKeyboardMarkup {
    let yes_txt = match lang {
        Lang::Ru => "✅ Да, удалить",
        Lang::En => "✅ Yes, delete",
    };
    InlineKeyboardMarkup::new(vec![vec![
        cb(yes_txt, &format!("delyes:{name}")),
        cb(&i18n::btn_back(lang), "menu"),
    ]])
}

pub fn confirm_recreate(lang: Lang, name: &str) -> InlineKeyboardMarkup {
    let yes_txt = match lang {
        Lang::Ru => "♻️ Пересоздать",
        Lang::En => "♻️ Recreate",
    };
    InlineKeyboardMarkup::new(vec![vec![
        cb(yes_txt, &format!("recreate:{name}")),
        cb(&i18n::btn_back(lang), "menu"),
    ]])
}

pub fn confirm_regen_all(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(&i18n::btn_regen_all_go(lang), "regen_all_go")],
        vec![cb(&i18n::btn_regen_all_routes(lang), "regen_all_routes")],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

pub fn modify_param_menu(lang: Lang, name: &str) -> InlineKeyboardMarkup {
    use crate::vpn::validate::ModifyParam;
    InlineKeyboardMarkup::new(vec![
        vec![
            cb(
                &i18n::modify_param_label(lang, ModifyParam::Keepalive),
                &format!("modparam:{name}:keepalive"),
            ),
            cb(
                &i18n::modify_param_label(lang, ModifyParam::Dns),
                &format!("modparam:{name}:dns"),
            ),
        ],
        vec![
            cb(
                &i18n::modify_param_label(lang, ModifyParam::AllowedIps),
                &format!("modparam:{name}:allowedips"),
            ),
            cb(
                &i18n::modify_param_label(lang, ModifyParam::Endpoint),
                &format!("modparam:{name}:endpoint"),
            ),
        ],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

pub fn confirm_restart_menu(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(&i18n::btn_restart_go(lang), "restart_go")],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

pub fn backup_menu(lang: Lang) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![cb(&i18n::btn_backup_new(lang), "bk:new")],
        vec![cb(&i18n::btn_backup_list(lang), "bk:list")],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

/// Один ряд на бэкап, кнопка ведёт на карточку по индексу в `list_backups()`.
/// Имя файла — обычный текст кнопки (Telegram не рендерит в кнопках HTML,
/// экранирование здесь не нужно, в отличие от текста сообщений).
pub fn backups_list(lang: Lang, backups: &[BackupFile]) -> InlineKeyboardMarkup {
    let mut rows: Vec<Vec<InlineKeyboardButton>> = backups
        .iter()
        .enumerate()
        .map(|(idx, bf)| vec![cb(&bf.name, &format!("bk:card:{idx}"))])
        .collect();
    rows.push(vec![cb(&i18n::btn_back(lang), "menu")]);
    InlineKeyboardMarkup::new(rows)
}

pub fn backup_card(lang: Lang, idx: usize) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![
        vec![
            cb(&i18n::btn_download(lang), &format!("bk:dl:{idx}")),
            cb(&i18n::btn_restore(lang), &format!("bk:restore:{idx}")),
        ],
        vec![cb(&i18n::btn_back(lang), "menu")],
    ])
}

pub fn confirm_restore(lang: Lang, idx: usize) -> InlineKeyboardMarkup {
    InlineKeyboardMarkup::new(vec![vec![
        cb(&i18n::btn_confirm(lang), &format!("bk:restore_yes:{idx}")),
        cb(&i18n::btn_back(lang), "menu"),
    ]])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_callback_data(kb: &InlineKeyboardMarkup) -> Vec<String> {
        kb.inline_keyboard
            .iter()
            .flatten()
            .filter_map(|b| match &b.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(d) => Some(d.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn main_menu_has_expected_actions() {
        let data = all_callback_data(&main_menu(Lang::Ru));
        for expected in [
            "list", "add", "addbulk", "stats", "backup", "check", "diagnose", "restart", "repair",
            "settings",
        ] {
            assert!(data.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn bulk_psk_step_emits_bulkadd_callbacks() {
        let data = all_callback_data(&bulk_psk_step(Lang::Ru, true));
        assert!(data.contains(&"bulkadd:psk:on".to_string()));
        assert!(!data.iter().any(|d| d.starts_with("add:psk:")));
    }

    #[test]
    fn main_menu_has_restart_and_repair() {
        let data = all_callback_data(&main_menu(Lang::Ru));
        assert!(data.contains(&"restart".to_string()));
        assert!(data.contains(&"repair".to_string()));
    }

    #[test]
    fn client_card_has_modify_button() {
        let data = all_callback_data(&client_card(Lang::Ru, "alice", true));
        assert!(data.contains(&"mod:alice".to_string()));
    }

    #[test]
    fn customer_can_confirm_self_refresh() {
        let card = all_callback_data(&customer_key_menu("alice"));
        assert!(card.contains(&"refresh:alice".to_string()));
        let confirm = all_callback_data(&customer_refresh_confirm_menu("alice"));
        assert!(confirm.contains(&"refreshgo:alice".to_string()));
        assert!(confirm.contains(&"mykey:alice".to_string()));
    }

    #[test]
    fn balance_purchase_and_renewal_require_explicit_confirmation() {
        let purchase = all_callback_data(&buy_balance_confirm_menu(12, 42));
        assert!(purchase.contains(&"buy:method:12:balance-go-42".to_string()));
        assert!(!purchase.contains(&"buy:method:12:balance".to_string()));

        let renewal = all_callback_data(&renew_balance_confirm_menu("alice", 3, 1_700_000_000));
        assert!(renewal.contains(&"renew:method:alice:3:balance-go-1700000000".to_string()));
        assert!(renewal.contains(&"mykey:alice".to_string()));
    }

    #[test]
    fn notification_menu_toggles_each_optional_category() {
        let enabled = all_callback_data(&notification_settings_menu(true, true));
        assert!(enabled.contains(&"guide:notify-expiry-off".to_string()));
        assert!(enabled.contains(&"guide:notify-maintenance-off".to_string()));

        let disabled = all_callback_data(&notification_settings_menu(false, false));
        assert!(disabled.contains(&"guide:notify-expiry-on".to_string()));
        assert!(disabled.contains(&"guide:notify-maintenance-on".to_string()));
    }

    #[test]
    fn modify_param_menu_has_four_params_and_back() {
        let data = all_callback_data(&modify_param_menu(Lang::Ru, "alice"));
        assert!(data.contains(&"modparam:alice:keepalive".to_string()));
        assert!(data.contains(&"modparam:alice:dns".to_string()));
        assert!(data.contains(&"modparam:alice:allowedips".to_string()));
        assert!(data.contains(&"modparam:alice:endpoint".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn confirm_restart_menu_has_go_and_back() {
        let data = all_callback_data(&confirm_restart_menu(Lang::Ru));
        assert!(data.contains(&"restart_go".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn expiry_menu_has_custom_and_presets() {
        let data = all_callback_data(&expiry_menu(Lang::Ru));
        assert!(data.contains(&"exp:none".to_string()));
        assert!(data.contains(&"exp:30d".to_string()));
        assert!(data.contains(&"exp:custom".to_string()));
    }

    #[test]
    fn client_card_encodes_name() {
        let data = all_callback_data(&client_card(Lang::Ru, "alice", false));
        assert!(data.contains(&"conf:alice".to_string()));
        assert!(data.contains(&"regen:alice".to_string()));
    }

    #[test]
    fn confirm_delete_encodes_name() {
        let data = all_callback_data(&confirm_delete(Lang::Ru, "bob"));
        assert!(data.contains(&"delyes:bob".to_string()));
    }

    #[test]
    fn confirm_recreate_encodes_name() {
        let data = all_callback_data(&confirm_recreate(Lang::Ru, "bob"));
        assert!(data.contains(&"recreate:bob".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn clients_list_has_regen_all_button() {
        let clients = vec![Client {
            name: "a".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        }];
        let data = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            true,
        ));
        assert!(data.contains(&"regen_all".to_string()));
    }

    #[test]
    fn clients_list_has_refresh_button() {
        // 🔄 «Обновить» эмитит `page:{page}` (Action::Page → edit-in-place с сохранением
        // текущей страницы). На странице 0 это `page:0`. Кнопка должна быть всегда —
        // даже на одностраничном списке (иначе обновить статусы нельзя).
        let clients = vec![Client {
            name: "a".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        }];
        let data = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(
            data.contains(&"page:0".to_string()),
            "refresh button (page:0) missing: {data:?}"
        );
    }

    #[test]
    fn clients_list_refresh_between_pagination() {
        // На многостраничном списке nav-ряд выглядит [◀️] [🔄] [▶️]:
        let clients: Vec<Client> = (0..20)
            .map(|i| Client {
                name: format!("c{i}"),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            })
            .collect();
        let kb = clients_list(Lang::Ru, &clients, &[], 0, 0, 8, ClientFilter::All, false);
        // nav-ряд — первый после клиентских (8 клиентов на странице → ряд с индексом 8).
        let nav_row = &kb.inline_keyboard[8];
        let data: Vec<&str> = nav_row
            .iter()
            .filter_map(|b| match &b.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(d) => Some(d.as_str()),
                _ => None,
            })
            .collect();
        // [🔄 page:0] [▶️ page:1] — refresh на странице 0 сохраняет её:
        assert_eq!(data, vec!["page:0", "page:1"]);
    }

    #[test]
    fn clients_list_refresh_preserves_page() {
        // 🔄 на странице N эмитит `page:N` — обновляет данные, не сбрасывая на 0.
        // 24 клиента / 8 на странице → 3 страницы; на странице 2 nav-ряд:
        // [◀️ page:1] [🔄 page:2] (▶️ нет, т.к. последняя страница).
        let clients: Vec<Client> = (0..24)
            .map(|i| Client {
                name: format!("c{i}"),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            })
            .collect();
        let kb = clients_list(Lang::Ru, &clients, &[], 0, 2, 8, ClientFilter::All, false);
        // Страница 2: клиентские ряды 16..23 (8 шт.) → nav-ряд с индексом 8.
        let nav_row = &kb.inline_keyboard[8];
        let data: Vec<&str> = nav_row
            .iter()
            .filter_map(|b| match &b.kind {
                teloxide::types::InlineKeyboardButtonKind::CallbackData(d) => Some(d.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(data, vec!["page:1", "page:2"]);
    }

    #[test]
    fn confirm_regen_all_has_three_actions() {
        let data = all_callback_data(&confirm_regen_all(Lang::Ru));
        assert!(data.contains(&"regen_all_go".to_string()));
        assert!(data.contains(&"regen_all_routes".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    fn all_button_texts(kb: &InlineKeyboardMarkup) -> Vec<String> {
        kb.inline_keyboard
            .iter()
            .flatten()
            .map(|b| b.text.clone())
            .collect()
    }

    #[test]
    fn clients_list_one_button_per_client() {
        let clients = vec![
            Client {
                name: "a".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
            Client {
                name: "b".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "inactive".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
        ];
        let data = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(data.contains(&"client:a".to_string()));
        assert!(data.contains(&"client:b".to_string()));
    }

    #[test]
    fn clients_list_zero_per_page_no_panic() {
        // Test with empty clients
        let empty_clients: Vec<Client> = vec![];
        let kb_empty = clients_list(
            Lang::Ru,
            &empty_clients,
            &[],
            0,
            0,
            0,
            ClientFilter::All,
            false,
        );
        let data_empty = all_callback_data(&kb_empty);
        assert_eq!(
            data_empty,
            vec!["menu"],
            "empty clients with per_page=0 should have only menu callback"
        );

        // Test with non-empty clients
        let clients = vec![
            Client {
                name: "a".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
            Client {
                name: "b".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "inactive".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
        ];
        let kb_filled = clients_list(Lang::Ru, &clients, &[], 0, 0, 0, ClientFilter::All, false);
        let data_filled = all_callback_data(&kb_filled);
        assert_eq!(
            data_filled,
            vec!["menu"],
            "non-empty clients with per_page=0 should have only menu callback"
        );
    }

    #[test]
    fn clients_list_shows_expiry_badge() {
        let clients = vec![
            Client {
                name: "temp".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
            Client {
                name: "perm".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
        ];
        let now = 1_700_000_000;
        let expiries = vec![Some(now + 6 * 86400), None];
        let texts = all_button_texts(&clients_list(
            Lang::Ru,
            &clients,
            &expiries,
            now,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(
            texts
                .iter()
                .any(|t| t.contains("temp") && t.contains("⏳ 6д")),
            "temp должен иметь метку: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("perm") && !t.contains("⏳")),
            "perm должен быть без метки: {texts:?}"
        );
    }

    #[test]
    fn clients_list_three_color_marks_by_handshake() {
        // 🟢 недавний handshake / 🟡 никогда не подключался / 🔴 handshake давно —
        // трёхцветная индикация, цвет считает бот из last_handshake (см. model::mark).
        let now = 1_700_000_000;
        let clients = vec![
            Client {
                name: "online".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: Some(now - 30), // недавно — онлайн
            },
            Client {
                name: "never".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "no_handshake".into(),
                rx: 0,
                tx: 0,
                last_handshake: None,
            },
            Client {
                name: "gone".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "inactive".into(),
                rx: 0,
                tx: 0,
                last_handshake: Some(now - 6 * 3600), // был, но давно
            },
        ];
        let texts = all_button_texts(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            now,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(
            texts.iter().any(|t| t.starts_with("🟢 online")),
            "active должен быть зелёным: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.starts_with("🟡 never")),
            "no_handshake должен быть жёлтым: {texts:?}"
        );
        assert!(
            texts.iter().any(|t| t.starts_with("🔴 gone")),
            "inactive должен быть красным: {texts:?}"
        );
    }

    #[test]
    fn clients_list_shows_compact_handshake() {
        // handshake в кнопке — компактно («10 мин», «никогда»); last_handshake
        // приходит из stats --json (экран списка переключён на stats).
        let now = 1_700_000_000;
        let clients = vec![
            Client {
                name: "recent".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "active".into(),
                rx: 0,
                tx: 0,
                last_handshake: Some(now - 600),
            },
            Client {
                name: "fresh".into(),
                ip: String::new(),
                client_ipv6: String::new(),
                status: String::new(),
                status_code: "no_handshake".into(),
                rx: 0,
                tx: 0,
                last_handshake: Some(0),
            },
        ];
        let texts = all_button_texts(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            now,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(
            texts
                .iter()
                .any(|t| t.contains("recent") && t.contains("10 мин")),
            "recent должен показывать handshake: {texts:?}"
        );
        assert!(
            texts
                .iter()
                .any(|t| t.contains("fresh") && t.contains("никогда")),
            "fresh (last_handshake=0) должен показывать «никогда»: {texts:?}"
        );
    }

    #[test]
    fn clients_list_has_filter_row_with_four_buttons() {
        // Ряд фильтра: [Все] [🟢 Онлайн] [🔴 Оффлайн] [🟡 Никогда] —
        // четыре callback `listfilter:*`.
        let clients = vec![Client {
            name: "a".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        }];
        let data = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(data.contains(&"listfilter:all".to_string()));
        assert!(data.contains(&"listfilter:online".to_string()));
        assert!(data.contains(&"listfilter:offline".to_string()));
        assert!(data.contains(&"listfilter:never".to_string()));
    }

    #[test]
    fn clients_list_marks_active_filter_with_checkmark() {
        // Активный фильтр помечается ✅-префиксом в подписи кнопки.
        let clients = vec![Client {
            name: "a".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        }];
        let texts_online = all_button_texts(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::Online,
            false,
        ));
        assert!(
            texts_online
                .iter()
                .any(|t| t.contains("✅") && t.contains("Подключены")),
            "активный фильтр Online должен иметь ✅: {texts_online:?}"
        );
        // Все остальные фильтры — без ✅
        assert!(
            texts_online
                .iter()
                .filter(|t| t.contains("Оффлайн"))
                .all(|t| !t.contains("✅")),
            "неактивные фильтры не должны иметь ✅: {texts_online:?}"
        );
    }

    #[test]
    fn clients_list_scope_button_gated_by_can_scope() {
        // Кнопка «🗂» (gscope) — только для владельца (can_scope=true);
        // групповому админу (can_scope=false) она не показывается.
        let clients = vec![Client {
            name: "a".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        }];
        let with_scope = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            true,
        ));
        assert!(with_scope.contains(&"gscope".to_string()));
        let without_scope = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(!without_scope.contains(&"gscope".to_string()));
    }

    #[test]
    fn clients_list_regen_all_only_for_owner() {
        // «♻️ Перевыпустить всех» — глобальное owner-only действие; групповому
        // админу кнопка не показывается (тап всё равно блокирует handler).
        let clients = vec![Client {
            name: "a".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        }];
        let owner = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            true,
        ));
        assert!(owner.contains(&"regen_all".to_string()));
        let ga = all_callback_data(&clients_list(
            Lang::Ru,
            &clients,
            &[],
            0,
            0,
            10,
            ClientFilter::All,
            false,
        ));
        assert!(!ga.contains(&"regen_all".to_string()));
    }

    #[test]
    fn client_card_modify_and_move_only_for_owner() {
        // «Изменить» и «Перенести» — owner-only: групповому админу обе кнопки
        // не показываются (их Action'ы и так гейтятся ролью).
        let owner = all_callback_data(&client_card(Lang::Ru, "alice", true));
        assert!(owner.contains(&"mod:alice".to_string()));
        assert!(owner.contains(&"gmove:alice".to_string()));
        let ga = all_callback_data(&client_card(Lang::Ru, "alice", false));
        assert!(!ga.contains(&"mod:alice".to_string()));
        assert!(!ga.contains(&"gmove:alice".to_string()));
    }

    #[test]
    fn group_scope_menu_encodes_all_none_and_groups() {
        let g = crate::store::GroupRow {
            id: 5,
            name: "family".into(),
            max_clients: None,
            created_at: 0,
        };
        let data = all_callback_data(&group_scope_menu(Lang::Ru, &[g]));
        assert!(data.contains(&"gscope:all".to_string()));
        assert!(data.contains(&"gscope:none".to_string()));
        assert!(data.contains(&"gscope:5".to_string()));
        assert!(data.contains(&"list".to_string()));
    }

    #[test]
    fn language_select_has_both_langs() {
        let data = all_callback_data(&language_select());
        assert!(data.contains(&"lang:ru".to_string()));
        assert!(data.contains(&"lang:en".to_string()));
    }

    #[test]
    fn settings_menu_toggles_psk_data_by_current_value() {
        let data_off = all_callback_data(&settings_menu(Lang::Ru, false, false, true, true, true));
        assert!(data_off.contains(&"set:psk:on".to_string()));
        assert!(!data_off.contains(&"set:psk:off".to_string()));
        let data_on = all_callback_data(&settings_menu(Lang::Ru, true, false, true, true, true));
        assert!(data_on.contains(&"set:psk:off".to_string()));
        assert!(data_on.contains(&"set:lang:ru".to_string()));
        assert!(data_on.contains(&"set:lang:en".to_string()));
        assert!(data_on.contains(&"admin:dashboard".to_string()));
    }

    #[test]
    fn client_card_has_four_artifact_buttons() {
        let data = all_callback_data(&client_card(Lang::Ru, "alice", false));
        assert!(data.contains(&"conf:alice".to_string()));
        assert!(data.contains(&"qr:alice".to_string()));
        assert!(data.contains(&"uri:alice".to_string()));
        assert!(data.contains(&"all:alice".to_string()));
        assert!(data.contains(&"del:alice".to_string()));
    }

    #[test]
    fn bulk_count_menu_has_presets_and_back() {
        let data = all_callback_data(&bulk_count_menu(Lang::Ru));
        // пресеты 1/3/5/10 — кодируем как bulk:N
        assert!(data.contains(&"bulk:1".to_string()));
        assert!(data.contains(&"bulk:3".to_string()));
        assert!(data.contains(&"bulk:5".to_string()));
        assert!(data.contains(&"bulk:10".to_string()));
        assert!(data.contains(&"bulk:20".to_string()));
        assert!(data.contains(&"bulk:50".to_string()));
        assert!(data.contains(&"bulk:99".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn settings_menu_has_deliver_toggles() {
        let data = all_callback_data(&settings_menu(Lang::Ru, true, false, true, false, true));
        assert!(data.contains(&"set:conf:off".to_string())); // on → эмитит off
        assert!(!data.contains(&"set:conf:on".to_string()));
        assert!(data.contains(&"set:qr:on".to_string())); // off → эмитит on
        assert!(data.contains(&"set:link:off".to_string())); // on → эмитит off
    }

    #[test]
    fn admin_dashboard_reaches_every_primary_section() {
        let data = all_callback_data(&admin_dashboard_menu());
        for expected in [
            "admin:servers",
            "admin:keys",
            "admin:users",
            "admin:finance",
            "stats",
            "admin:communication",
            "admin:system",
            "portal",
            "admin:search",
        ] {
            assert!(
                data.iter().any(|value| value == expected),
                "missing {expected}"
            );
        }
        let create = all_callback_data(&admin_create_menu());
        assert!(create.contains(&"add".to_string()));
        assert!(create.contains(&"addbulk".to_string()));
        let vpn = all_callback_data(&vpn_service_menu());
        for expected in [
            "check",
            "diagnose",
            "restart",
            "repair",
            "migration:local",
            "admin:dashboard",
        ] {
            assert!(vpn.contains(&expected.to_string()), "missing {expected}");
        }
        let migration = all_callback_data(&local_migration_menu());
        for expected in [
            "migration:preflight",
            "migration:start",
            "migration:status",
            "migration:rollback",
        ] {
            assert!(
                migration.contains(&expected.to_string()),
                "missing {expected}"
            );
        }
        assert!(all_callback_data(&finance_menu()).contains(&"admin:dashboard".to_string()));
        assert!(
            all_callback_data(&settings_menu(Lang::Ru, false, false, true, true, true))
                .contains(&"admin:dashboard".to_string())
        );
    }

    #[test]
    fn server_card_exposes_remote_install_and_health_actions() {
        let data = all_callback_data(&server_card_menu(42));
        for expected in [
            "server:deploy:42",
            "server:check:42",
            "server:diagnose:42",
            "server:maintenance:42",
            "server:maintenance:finish:42",
            "server:panel:42",
            "server:panel:sync:42",
            "server:default:42",
        ] {
            assert!(data.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn server_setup_wizard_offers_all_connection_methods() {
        let data = all_callback_data(&server_setup_method_menu(42));
        for expected in [
            "server:deploy:42",
            "server:panel:42",
            "server:enroll:42",
            "server:42",
        ] {
            assert!(data.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn operations_menu_offers_refresh_and_affected_servers() {
        let data = all_callback_data(&admin_operations_menu(&[2, 7], true));
        assert!(data.contains(&"admin:operations:refresh".to_string()));
        assert!(data.contains(&"admin:operations:ack".to_string()));
        assert!(data.contains(&"server:2".to_string()));
        assert!(data.contains(&"server:7".to_string()));
    }

    #[test]
    fn admin_user_keys_offer_open_delete_and_back() {
        let data = all_callback_data(&admin_user_keys_menu(7, &["alice-phone".into()]));
        assert!(data.contains(&"show:alice-phone".to_string()));
        assert!(data.contains(&"del:alice-phone".to_string()));
        assert!(data.contains(&"admin:user:7".to_string()));
    }

    #[test]
    fn remote_migration_requires_test_before_manual_enablement() {
        let data = all_callback_data(&remote_migration_menu(42));
        for expected in [
            "server:migrate:test:42",
            "server:migrate:approve:42",
            "server:migrate:rollback:42",
        ] {
            assert!(data.contains(&expected.to_string()), "missing {expected}");
        }
    }

    #[test]
    fn psk_step_has_both_options_and_back() {
        let data = all_callback_data(&psk_step(Lang::Ru, false));
        assert!(data.contains(&"add:psk:on".to_string()));
        assert!(data.contains(&"add:psk:off".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn bulk_expiry_menu_uses_bulkexp_prefix() {
        // Копия expiry_menu, но все callback'и под `bulkexp:` — bulk-поток идёт
        // через отдельный Action без условной логики в общем обработчике Expiry.
        let data = all_callback_data(&bulk_expiry_menu(Lang::Ru));
        assert!(data.contains(&"bulkexp:none".to_string()));
        assert!(data.contains(&"bulkexp:1d".to_string()));
        assert!(data.contains(&"bulkexp:30d".to_string()));
        assert!(data.contains(&"bulkexp:365d".to_string()));
        assert!(data.contains(&"bulkexp:custom".to_string()));
        // Подтверждаем, что bulk-экран НЕ пересекается с одиночным потоком:
        assert!(!data.iter().any(|d| d.starts_with("exp:")));
    }

    #[test]
    fn backup_menu_has_new_list_and_back() {
        let data = all_callback_data(&backup_menu(Lang::Ru));
        assert!(data.contains(&"bk:new".to_string()));
        assert!(data.contains(&"bk:list".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn backups_list_one_button_per_backup_by_index() {
        let backups = vec![
            BackupFile {
                name: "a.tar.gz".into(),
                path: "a.tar.gz".into(),
                size: 1,
                mtime: 1,
            },
            BackupFile {
                name: "b.tar.gz".into(),
                path: "b.tar.gz".into(),
                size: 2,
                mtime: 2,
            },
        ];
        let data = all_callback_data(&backups_list(Lang::Ru, &backups));
        assert!(data.contains(&"bk:card:0".to_string()));
        assert!(data.contains(&"bk:card:1".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn backup_card_encodes_index() {
        let data = all_callback_data(&backup_card(Lang::Ru, 2));
        assert!(data.contains(&"bk:dl:2".to_string()));
        assert!(data.contains(&"bk:restore:2".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn confirm_restore_encodes_index() {
        let data = all_callback_data(&confirm_restore(Lang::Ru, 3));
        assert!(data.contains(&"bk:restore_yes:3".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    fn g(id: i64, name: &str) -> crate::store::GroupRow {
        crate::store::GroupRow {
            id,
            name: name.into(),
            max_clients: None,
            created_at: 0,
        }
    }

    #[test]
    fn group_keyboards_emit_expected_callbacks() {
        let kb = groups_menu(Lang::Ru, &[(g(1, "family"), 3)]);
        let data = all_callback_data(&kb);
        assert!(data.contains(&"g:card:1".to_string()));
        assert!(data.contains(&"g:new".to_string()));

        let card = group_card_menu(Lang::Ru, 7, true);
        let card_data = all_callback_data(&card);
        for expected in [
            "g:ren:7",
            "g:quota:7",
            "g:adm:7",
            "g:invrev:7",
            "g:del:7",
            "g:regen:7",
        ] {
            assert!(card_data.contains(&expected.to_string()), "нет {expected}");
        }
        // has_invite=false → вместо отзыва кнопка создания
        let card2 = group_card_menu(Lang::Ru, 7, false);
        let d2 = format!("{card2:?}");
        assert!(d2.contains("g:inv:7"));

        let mv = move_client_menu(Lang::Ru, "alice", &[g(1, "family")]);
        let mv_dbg = format!("{mv:?}");
        assert!(mv_dbg.contains("gmoveto:1:alice"));
        assert!(mv_dbg.contains("gmoveto:none:alice"));
    }

    #[test]
    fn main_menu_has_groups_button() {
        let dbg = format!("{:?}", main_menu(Lang::Ru));
        assert!(dbg.contains("\"groups\""));
    }

    #[test]
    fn ga_menu_switch_only_when_multi() {
        assert!(format!("{:?}", ga_main_menu(Lang::Ru, true)).contains("g:selmenu"));
        assert!(!format!("{:?}", ga_main_menu(Lang::Ru, false)).contains("g:selmenu"));
    }

    #[test]
    fn psk_step_default_option_listed_first() {
        let kb_off = psk_step(Lang::Ru, false);
        let first_row_off = &kb_off.inline_keyboard[0];
        match &first_row_off[0].kind {
            teloxide::types::InlineKeyboardButtonKind::CallbackData(d) => {
                assert_eq!(d, "add:psk:off")
            }
            _ => panic!("expected callback data"),
        }

        let kb_on = psk_step(Lang::Ru, true);
        let first_row_on = &kb_on.inline_keyboard[0];
        match &first_row_on[0].kind {
            teloxide::types::InlineKeyboardButtonKind::CallbackData(d) => {
                assert_eq!(d, "add:psk:on")
            }
            _ => panic!("expected callback data"),
        }
    }

    #[test]
    fn clients_empty_menu_keeps_filter_and_scope_controls() {
        // Тупик #20: пустая выборка при липком фильтре/скоупе обязана
        // оставлять кнопки смены статус-фильтра и группового фильтра —
        // иначе «Без группы» без клиентов запирает раздел клиентов.
        let data = all_callback_data(&clients_empty_menu(Lang::Ru, ClientFilter::All, true));
        assert!(data.contains(&"gscope".to_string()));
        assert!(data.contains(&"listfilter:all".to_string()));
        assert!(data.contains(&"menu".to_string()));
    }

    #[test]
    fn clients_empty_menu_hides_scope_for_group_admin() {
        let data = all_callback_data(&clients_empty_menu(Lang::Ru, ClientFilter::All, false));
        assert!(!data.contains(&"gscope".to_string()));
        assert!(data.contains(&"listfilter:all".to_string()));
    }

    #[test]
    fn group_card_menu_has_group_clients_button() {
        // «Клиенты группы» (#20): открывает список клиентов с фильтром группы.
        let data = all_callback_data(&group_card_menu(Lang::Ru, 7, false));
        assert!(data.contains(&"gscope:7".to_string()));
    }
}
