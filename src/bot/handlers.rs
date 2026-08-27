use std::sync::Arc;

use teloxide::dispatching::dialogue::InMemStorage;
use teloxide::dispatching::{HandlerExt, UpdateFilterExt};
use teloxide::prelude::*;
use teloxide::types::{
    CallbackQuery, InlineKeyboardMarkup, InputFile, LabeledPrice, MessageId, ParseMode,
    PreCheckoutQuery,
};

use crate::auth::{resolve_role, Role};
use crate::bot::menu;
use crate::bot::render::{self, format_client_card, format_stats};
use crate::bot::State;
use crate::config::Config;
use crate::i18n::{self, Lang};
use crate::store::{EventKind, ListScope, Store};
use crate::vpn::Vpn;

#[derive(Debug, PartialEq)]
pub enum Action {
    AdminDashboard,
    AdminVpn,
    AdminServers,
    AdminKeys,
    AdminUsersHub,
    AdminCommunication,
    AdminSystem,
    AdminUpdate,
    AdminUpdateRun,
    AdminUpdateStatus,
    AdminUpdateRollback,
    ServerAdd,
    ServerCard(i64),
    RemoteMigration(i64),
    RemoteMigrationPreflight(i64),
    RemoteMigrationStatus(i64),
    RemoteMigrationTest(i64),
    RemoteMigrationApprove(i64),
    RemoteMigrationAsk(i64),
    RemoteMigrationRun(i64),
    RemoteMigrationRollback(i64),
    ServerBilling,
    ServerBillingAsk(i64),
    ServerPassportAsk(i64),
    ServerEnroll(i64),
    ServerEnrollRevoke(i64),
    ServerSetDefault(i64),
    ServerDeployAsk(i64),
    ServerCheck(i64),
    ServerDiagnose(i64),
    ServerProvisioningProbe(i64),
    ServerPanelConnect(i64),
    ServerPanelSync(i64),
    ServerPanelAudit(i64),
    LocalMigration,
    LocalMigrationPreflight,
    LocalMigrationStart,
    LocalMigrationStatus,
    LocalMigrationRollback,
    AdminCreate,
    AdminOwners,
    AdminOwnersPage(usize),
    AdminFinance,
    AdminSupport,
    AdminBroadcast,
    AdminBroadcastTemplates,
    BroadcastAudience(String),
    BroadcastRetry(i64),
    AdminHelp,
    AdminSearch,
    AdminRoles,
    AdminRoleAction(String),
    AdminBulk(String),
    AdminBulkConfirm,
    AdminUser(i64),
    AdminUserKeys(i64),
    AdminUserDeleteKeysAsk(i64),
    AdminUserDeleteKeysConfirm(i64),
    AdminUserPayments(i64),
    AdminUserBalance(i64),
    AdminUserDiscount(i64),
    AdminUserNote(i64),
    AdminUserBlock(i64, bool),
    StatsSection(String),
    SupportFilter(String),
    AdminPromos,
    AdminCommerce,
    AdminPricesRub,
    AdminPricesStars,
    AdminReferral,
    AdminPromoAction(String),
    ClientNoteAsk(String),
    LegacyRenew(String),
    LegacyRenewMethod(String, String),
    LegacyRestore,
    LegacyRequestNew,
    LegacyRequestApprove(i64),
    LegacyRequestReject(i64),
    PromoInput,
    Guide(String),
    LegacyPriceAsk,
    Menu,
    List,
    Add,
    Stats,
    Page(usize),
    ShowClient(String),
    ClientHistory(String),
    SendConf(String),
    AskDelete(String),
    ConfirmDelete(String),
    Recreate(String),
    Regen(String),
    RegenAll,
    RegenAllRun(bool), // true = --reset-routes
    Expiry(String),    // "none" | "1d" | ... | "custom"
    Lang(String),      // "ru" | "en" — язык-гейт при первом /start
    Settings,
    SetLang(String), // "ru" | "en" — смена языка из экрана настроек
    SetPsk(bool),
    SetSlug(bool),
    AddPsk(bool),
    AddServer(i64),
    Backup,
    BackupNew,
    BackupList,
    BackupCard(usize),
    BackupDownload(usize),
    Restore(usize),
    RestoreYes(usize),
    Check,
    Diagnose,
    Modify(String),
    ModifyParam(String, crate::vpn::validate::ModifyParam),
    Restart,
    RestartRun,
    RepairModule,
    // --- Массовая генерация (#22) ---
    AddBulk,
    AddBulkRun(usize), // N клиентов для генерации (1..=MAX_BULK, валидируется в обработчике)
    BulkExpiry(String), // "none" | "1d" | ... | "custom" — общий срок для всей пачки
    AddBulkPsk(bool),  // true = включить PSK для генерируемых клиентов
    BulkServer(i64),
    // --- Артефакты существующего клиента (повторная выдача) ---
    SendQr(String),
    SendLink(String),
    SendAll(String),
    // --- Тумблеры выдачи артефактов в настройках ---
    SetConf(bool),
    SetQr(bool),
    SetLink(bool),
    // --- Фильтр списка клиентов (#28) ---
    SetListFilter(crate::vpn::model::ClientFilter),
    // --- Группы (#20): делегирование управления групповым админам ---
    Groups,
    GroupCreate,
    GroupCard(i64),
    GroupRenameAsk(i64),
    GroupQuotaAsk(i64),
    GroupAdmins(i64),
    GroupAdminRemove(i64, i64),
    GroupInvite(i64),
    GroupInviteRevoke(i64),
    GroupAdminById(i64),
    GroupDeleteAsk(i64),
    GroupDeleteDetach(i64),
    GroupDeleteAllAsk(i64),
    GroupDeleteAllYes(i64),
    GroupRegenAsk(i64),
    GroupRegenRun(i64),
    GroupSelect(i64),
    GroupSelectMenu,
    MoveClientAsk(String),
    MoveClientTo(Option<i64>, String),
    GroupScopeAsk,
    GroupScopeSet(ListScope),
    Buy,
    BuyServer(i64),
    BuyTerm(i64),
    BuyMethod(i64, String),
    BuyPaid(i64),
    MyKeys,
    Profile,
    Portal,
    Balance,
    PaymentApprove(i64),
    PaymentReject(i64),
    AssignOwnerAsk(String),
    AdminExpiryAsk(String),
    SetClientEnabled(String, bool),
    PaymentInstructionsAsk,
    AcquiringUrlAsk,
    CustomerKey(String),
    CustomerMove(String),
    CustomerMoveServer(String, i64),
    CustomerMoveConfirm(i64),
    CustomerMoveCancel(i64),
    CustomerRefresh(String),
    CustomerRefreshRun(String),
    Renew(String),
    RenewTerm(String, i64),
    RenewMethod(String, i64, String),
    AutoRenew(String, i64, bool),
    DeviceLabelAsk(String),
    SupportTicket(i64),
    SupportNewCategory(String),
    SupportTake(i64),
    SupportReply(i64),
    SupportClose(i64),
    SupportPriority(i64, String),
    SupportRate(i64, i64),
    FinanceExport,
    Unknown,
}

fn parse_callback(data: &str) -> Action {
    match data {
        "admin:dashboard" => Action::AdminDashboard,
        "admin:vpn" => Action::AdminVpn,
        "admin:servers" => Action::AdminServers,
        "migration:local" => Action::LocalMigration,
        "migration:preflight" => Action::LocalMigrationPreflight,
        "migration:start" => Action::LocalMigrationStart,
        "migration:status" => Action::LocalMigrationStatus,
        "migration:rollback" => Action::LocalMigrationRollback,
        "admin:keys" => Action::AdminKeys,
        "admin:users" => Action::AdminUsersHub,
        "admin:communication" => Action::AdminCommunication,
        "admin:system" => Action::AdminSystem,
        "admin:update" => Action::AdminUpdate,
        "admin:update:run" => Action::AdminUpdateRun,
        "admin:update:status" => Action::AdminUpdateStatus,
        "admin:update:rollback" => Action::AdminUpdateRollback,
        "server:add" => Action::ServerAdd,
        "server:billing" => Action::ServerBilling,
        "admin:create" => Action::AdminCreate,
        "admin:owners" => Action::AdminOwners,
        "admin:finance" => Action::AdminFinance,
        "admin:support" => Action::AdminSupport,
        "admin:broadcast" => Action::AdminBroadcast,
        "admin:broadcast:templates" => Action::AdminBroadcastTemplates,
        "admin:help" => Action::AdminHelp,
        "admin:search" => Action::AdminSearch,
        "admin:roles" => Action::AdminRoles,
        "menu" => Action::Menu,
        "list" => Action::List,
        "add" => Action::Add,
        "addbulk" => Action::AddBulk,
        "stats" => Action::Stats,
        "admin:bulk:confirm" => Action::AdminBulkConfirm,
        "settings" => Action::Settings,
        "backup" => Action::Backup,
        "bk:new" => Action::BackupNew,
        "bk:list" => Action::BackupList,
        "check" => Action::Check,
        "diagnose" => Action::Diagnose,
        "regen_all" => Action::RegenAll,
        "regen_all_go" => Action::RegenAllRun(false),
        "regen_all_routes" => Action::RegenAllRun(true),
        "restart" => Action::Restart,
        "restart_go" => Action::RestartRun,
        "repair" => Action::RepairModule,
        "groups" => Action::Groups,
        "g:new" => Action::GroupCreate,
        "g:selmenu" => Action::GroupSelectMenu,
        "gscope" => Action::GroupScopeAsk,
        "buy" => Action::Buy,
        "mykeys" => Action::MyKeys,
        "profile" => Action::Profile,
        "portal" => Action::Portal,
        "balance" => Action::Balance,
        "set:payment" => Action::PaymentInstructionsAsk,
        "set:acquiring" => Action::AcquiringUrlAsk,
        "finance:export" => Action::FinanceExport,
        "admin:promos" => Action::AdminPromos,
        "admin:commerce" => Action::AdminCommerce,
        "admin:prices:rub" => Action::AdminPricesRub,
        "admin:prices:stars" => Action::AdminPricesStars,
        "admin:referral" => Action::AdminReferral,
        "admin:legacy" => Action::LegacyRestore,
        "legacy:request:new" => Action::LegacyRequestNew,
        "legacy:promo" => Action::PromoInput,
        "legacy:price" => Action::LegacyPriceAsk,
        _ => {
            if let Some(v) = data.strip_prefix("guide:") {
                Action::Guide(v.to_string())
            } else if let Some(v) = data.strip_prefix("admin:owners:page:") {
                v.parse()
                    .map(Action::AdminOwnersPage)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:preflight:") {
                v.parse()
                    .map(Action::RemoteMigrationPreflight)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:status:") {
                v.parse()
                    .map(Action::RemoteMigrationStatus)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:test:") {
                v.parse()
                    .map(Action::RemoteMigrationTest)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:approve:") {
                v.parse()
                    .map(Action::RemoteMigrationApprove)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:ask:") {
                v.parse()
                    .map(Action::RemoteMigrationAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:run:") {
                v.parse()
                    .map(Action::RemoteMigrationRun)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:rollback:") {
                v.parse()
                    .map(Action::RemoteMigrationRollback)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:migrate:") {
                v.parse()
                    .map(Action::RemoteMigration)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("buy:term:") {
                v.parse().map(Action::BuyTerm).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("buy:method:") {
                let mut parts = v.splitn(2, ':');
                match (parts.next().and_then(|p| p.parse().ok()), parts.next()) {
                    (Some(months), Some(method)) => Action::BuyMethod(months, method.to_string()),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("buy:server:") {
                v.parse().map(Action::BuyServer).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("buy:paid:") {
                v.parse().map(Action::BuyPaid).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("pay:ok:") {
                v.parse()
                    .map(Action::PaymentApprove)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("pay:no:") {
                v.parse()
                    .map(Action::PaymentReject)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("owner:assign:") {
                Action::AssignOwnerAsk(v.to_string())
            } else if let Some(v) = data.strip_prefix("owner:expiry:") {
                Action::AdminExpiryAsk(v.to_string())
            } else if let Some(v) = data.strip_prefix("owner:enable:") {
                Action::SetClientEnabled(v.to_string(), true)
            } else if let Some(v) = data.strip_prefix("owner:disable:") {
                Action::SetClientEnabled(v.to_string(), false)
            } else if let Some(v) = data.strip_prefix("mykey:") {
                Action::CustomerKey(v.to_string())
            } else if let Some(v) = data.strip_prefix("move:choose:") {
                Action::CustomerMove(v.to_string())
            } else if let Some(v) = data.strip_prefix("move:run:") {
                let mut parts = v.rsplitn(2, ':');
                match (parts.next().and_then(|id| id.parse().ok()), parts.next()) {
                    (Some(server_id), Some(name)) => {
                        Action::CustomerMoveServer(name.to_string(), server_id)
                    }
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("move:confirm:") {
                v.parse()
                    .map(Action::CustomerMoveConfirm)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("move:cancel:") {
                v.parse()
                    .map(Action::CustomerMoveCancel)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("device:label:") {
                Action::DeviceLabelAsk(v.to_string())
            } else if let Some(v) = data.strip_prefix("support:take:") {
                v.parse()
                    .map(Action::SupportTake)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("support:new:") {
                Action::SupportNewCategory(v.to_string())
            } else if let Some(v) = data.strip_prefix("support:reply:") {
                v.parse()
                    .map(Action::SupportReply)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("support:close:") {
                v.parse()
                    .map(Action::SupportClose)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("support:priority:") {
                let mut p = v.splitn(2, ':');
                match (p.next().and_then(|x| x.parse().ok()), p.next()) {
                    (Some(id), Some(priority)) => Action::SupportPriority(id, priority.into()),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("support:rate:") {
                let mut p = v.splitn(2, ':');
                match (
                    p.next().and_then(|x| x.parse().ok()),
                    p.next().and_then(|x| x.parse().ok()),
                ) {
                    (Some(id), Some(rating)) => Action::SupportRate(id, rating),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("support:ticket:") {
                v.parse()
                    .map(Action::SupportTicket)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("support:filter:") {
                Action::SupportFilter(v.to_string())
            } else if let Some(v) = data.strip_prefix("client:note:") {
                Action::ClientNoteAsk(v.to_string())
            } else if let Some(v) = data.strip_prefix("admin:bulk:") {
                Action::AdminBulk(v.to_string())
            } else if let Some(v) = data.strip_prefix("admin:role:") {
                Action::AdminRoleAction(v.to_string())
            } else if let Some(v) = data.strip_prefix("admin:promo:") {
                Action::AdminPromoAction(v.to_string())
            } else if let Some(v) = data.strip_prefix("broadcast:audience:") {
                Action::BroadcastAudience(v.to_string())
            } else if let Some(v) = data.strip_prefix("broadcast:retry:") {
                v.parse()
                    .map(Action::BroadcastRetry)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:userblock:") {
                let mut p = v.split(':');
                match (p.next().and_then(|x| x.parse().ok()), p.next()) {
                    (Some(id), Some(flag)) => Action::AdminUserBlock(id, flag == "on"),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("admin:userkeys:") {
                v.parse()
                    .map(Action::AdminUserKeys)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:userkeys-delete-confirm:") {
                v.parse()
                    .map(Action::AdminUserDeleteKeysConfirm)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:userkeys-delete:") {
                v.parse()
                    .map(Action::AdminUserDeleteKeysAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:userpay:") {
                v.parse()
                    .map(Action::AdminUserPayments)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:userbal:") {
                v.parse()
                    .map(Action::AdminUserBalance)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:userdiscount:") {
                v.parse()
                    .map(Action::AdminUserDiscount)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:usernote:") {
                v.parse()
                    .map(Action::AdminUserNote)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("admin:user:") {
                v.parse().map(Action::AdminUser).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("stats:") {
                Action::StatsSection(v.to_string())
            } else if let Some(v) = data.strip_prefix("renew:term:") {
                let mut parts = v.rsplitn(2, ':');
                match (parts.next().and_then(|p| p.parse().ok()), parts.next()) {
                    (Some(months), Some(name)) => Action::RenewTerm(name.to_string(), months),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("legacy:renew:method:") {
                let mut parts = v.rsplitn(2, ':');
                match (parts.next(), parts.next()) {
                    (Some(method), Some(name)) => {
                        Action::LegacyRenewMethod(name.into(), method.into())
                    }
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("legacy:renew:") {
                Action::LegacyRenew(v.to_string())
            } else if let Some(v) = data.strip_prefix("legacy:req:ok:") {
                v.parse()
                    .map(Action::LegacyRequestApprove)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("legacy:req:no:") {
                v.parse()
                    .map(Action::LegacyRequestReject)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:bill:") {
                v.parse()
                    .map(Action::ServerBillingAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:edit:") {
                v.parse()
                    .map(Action::ServerPassportAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:enroll:revoke:") {
                v.parse()
                    .map(Action::ServerEnrollRevoke)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:default:") {
                v.parse()
                    .map(Action::ServerSetDefault)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:deploy:") {
                v.parse()
                    .map(Action::ServerDeployAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:check:") {
                v.parse()
                    .map(Action::ServerCheck)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:diagnose:") {
                v.parse()
                    .map(Action::ServerDiagnose)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:probe:") {
                v.parse()
                    .map(Action::ServerProvisioningProbe)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:panel:sync:") {
                v.parse()
                    .map(Action::ServerPanelSync)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:panel:audit:") {
                v.parse()
                    .map(Action::ServerPanelAudit)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:panel:") {
                v.parse()
                    .map(Action::ServerPanelConnect)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:enroll:") {
                v.parse()
                    .map(Action::ServerEnroll)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("server:") {
                v.parse().map(Action::ServerCard).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("add:server:") {
                v.parse().map(Action::AddServer).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("renew:method:") {
                let parts: Vec<_> = v.rsplitn(3, ':').collect();
                match (
                    parts.first(),
                    parts.get(1).and_then(|p| p.parse().ok()),
                    parts.get(2),
                ) {
                    (Some(method), Some(months), Some(name)) => {
                        Action::RenewMethod((*name).to_string(), months, (*method).to_string())
                    }
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("renew:") {
                Action::Renew(v.to_string())
            } else if let Some(v) = data.strip_prefix("autorenew:") {
                let parts: Vec<_> = v.rsplitn(3, ':').collect();
                match (
                    parts.first(),
                    parts.get(1).and_then(|p| p.parse().ok()),
                    parts.get(2),
                ) {
                    (Some(flag), Some(months), Some(name)) => {
                        Action::AutoRenew((*name).to_string(), months, *flag == "on")
                    }
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("g:card:") {
                v.parse().map(Action::GroupCard).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:ren:") {
                v.parse()
                    .map(Action::GroupRenameAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:quota:") {
                v.parse()
                    .map(Action::GroupQuotaAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:admdel:") {
                // ДО g:adm: (g:admdel:… тоже начинается с g:adm) — как delyes:/del:.
                let parts: Vec<&str> = v.splitn(2, ':').collect();
                match (
                    parts.first().and_then(|p| p.parse().ok()),
                    parts.get(1).and_then(|p| p.parse().ok()),
                ) {
                    (Some(g), Some(u)) => Action::GroupAdminRemove(g, u),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("g:admid:") {
                // ДО g:adm: — тот же принцип.
                v.parse()
                    .map(Action::GroupAdminById)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:adm:") {
                v.parse()
                    .map(Action::GroupAdmins)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:invrev:") {
                // ДО g:inv: — тот же принцип.
                v.parse()
                    .map(Action::GroupInviteRevoke)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:inv:") {
                v.parse()
                    .map(Action::GroupInvite)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:delallyes:") {
                // ДО g:delall: — тот же принцип.
                v.parse()
                    .map(Action::GroupDeleteAllYes)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:delall:") {
                // ДО g:del: — тот же принцип.
                v.parse()
                    .map(Action::GroupDeleteAllAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:deldetach:") {
                // ДО g:del: — тот же принцип.
                v.parse()
                    .map(Action::GroupDeleteDetach)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:del:") {
                v.parse()
                    .map(Action::GroupDeleteAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:regengo:") {
                // ДО g:regen: — тот же принцип.
                v.parse()
                    .map(Action::GroupRegenRun)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:regen:") {
                v.parse()
                    .map(Action::GroupRegenAsk)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("g:sel:") {
                v.parse()
                    .map(Action::GroupSelect)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("gmoveto:") {
                // ДО gmove: (gmoveto:… начинается с gmove) — как delyes:/del:.
                let parts: Vec<&str> = v.splitn(2, ':').collect();
                match (parts.first(), parts.get(1)) {
                    (Some(&"none"), Some(name)) => Action::MoveClientTo(None, name.to_string()),
                    (Some(id), Some(name)) => id
                        .parse()
                        .map(|g| Action::MoveClientTo(Some(g), name.to_string()))
                        .unwrap_or(Action::Unknown),
                    _ => Action::Unknown,
                }
            } else if let Some(v) = data.strip_prefix("gmove:") {
                Action::MoveClientAsk(v.to_string())
            } else if let Some(v) = data.strip_prefix("gscope:") {
                match v {
                    "all" => Action::GroupScopeSet(crate::store::ListScope::All),
                    "none" => Action::GroupScopeSet(crate::store::ListScope::NoGroup),
                    id => id
                        .parse()
                        .map(|g| Action::GroupScopeSet(crate::store::ListScope::Group(g)))
                        .unwrap_or(Action::Unknown),
                }
            } else if let Some(v) = data.strip_prefix("page:") {
                v.parse().map(Action::Page).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("client:") {
                Action::ShowClient(v.to_string())
            } else if let Some(v) = data.strip_prefix("conf:") {
                Action::SendConf(v.to_string())
            } else if let Some(v) = data.strip_prefix("qr:") {
                Action::SendQr(v.to_string())
            } else if let Some(v) = data.strip_prefix("uri:") {
                Action::SendLink(v.to_string())
            } else if let Some(v) = data.strip_prefix("all:") {
                Action::SendAll(v.to_string())
            } else if let Some(v) = data.strip_prefix("history:") {
                // "history" ничей не префикс среди существующих веток и сам не
                // конфликтует ни с одной из них — порядок относительно соседей
                // произвольный.
                Action::ClientHistory(v.to_string())
            } else if let Some(v) = data.strip_prefix("bulkadd:psk:") {
                // Must be checked before "bulk:" — same reason as delyes:/del:
                // ("bulkadd:..." also starts with "bulk", so "bulk:" would
                // prefix-match it and misparse as AddBulkRun("add:psk:on")).
                Action::AddBulkPsk(v == "on")
            } else if let Some(v) = data.strip_prefix("bulkserver:") {
                v.parse().map(Action::BulkServer).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("bulkexp:") {
                // Must be checked before "bulk:" — same reason as delyes:/del:
                // ("bulkexp:..." also starts with "bulk").
                Action::BulkExpiry(v.to_string())
            } else if let Some(v) = data.strip_prefix("bulk:") {
                // Проверяется ПОСЛЕ bulkadd:/bulkexp: (см. выше).
                v.parse().map(Action::AddBulkRun).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("delyes:") {
                // Must be checked before "del:" — otherwise "del:" prefix-matches
                // "delyes:..." and confirmed deletes get misparsed as delete-asks.
                Action::ConfirmDelete(v.to_string())
            } else if let Some(v) = data.strip_prefix("del:") {
                Action::AskDelete(v.to_string())
            } else if let Some(v) = data.strip_prefix("recreate:") {
                Action::Recreate(v.to_string())
            } else if let Some(v) = data.strip_prefix("refreshgo:") {
                Action::CustomerRefreshRun(v.to_string())
            } else if let Some(v) = data.strip_prefix("refresh:") {
                Action::CustomerRefresh(v.to_string())
            } else if let Some(v) = data.strip_prefix("regen:") {
                Action::Regen(v.to_string())
            } else if let Some(v) = data.strip_prefix("exp:") {
                Action::Expiry(v.to_string())
            } else if let Some(v) = data.strip_prefix("add:psk:") {
                // No collision with the exact-match "add" arm above (that's a
                // full-string match, not a prefix), but kept ahead of any
                // future generic "add:" prefix for the same reason as
                // delyes:/del: and set:lang:/lang: below.
                Action::AddPsk(v == "on")
            } else if let Some(v) = data.strip_prefix("set:lang:") {
                // Must be checked before the general "lang:" prefix — same reason
                // as delyes:/del: above ("set:lang:ru" also starts with "set:").
                Action::SetLang(v.to_string())
            } else if let Some(v) = data.strip_prefix("set:psk:") {
                Action::SetPsk(v == "on")
            } else if let Some(v) = data.strip_prefix("set:slug:") {
                Action::SetSlug(v == "on")
            } else if let Some(v) = data.strip_prefix("set:conf:") {
                Action::SetConf(v == "on")
            } else if let Some(v) = data.strip_prefix("set:qr:") {
                Action::SetQr(v == "on")
            } else if let Some(v) = data.strip_prefix("set:link:") {
                Action::SetLink(v == "on")
            } else if let Some(v) = data.strip_prefix("listfilter:") {
                // Фильтр списка клиентов (#28). "list" — точный match выше (не
                // префикс), так что listfilter: с ним не коллизирует.
                crate::vpn::model::ClientFilter::parse_str(v)
                    .map(Action::SetListFilter)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("lang:") {
                Action::Lang(v.to_string())
            } else if let Some(v) = data.strip_prefix("bk:restore_yes:") {
                // Must be checked before "bk:restore:" — otherwise "bk:restore:"
                // prefix-matches "bk:restore_yes:..." and confirmed restores get
                // misparsed as restore-asks (same pattern as delyes:/del:).
                v.parse().map(Action::RestoreYes).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("bk:restore:") {
                v.parse().map(Action::Restore).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("bk:card:") {
                v.parse().map(Action::BackupCard).unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("bk:dl:") {
                v.parse()
                    .map(Action::BackupDownload)
                    .unwrap_or(Action::Unknown)
            } else if let Some(v) = data.strip_prefix("modparam:") {
                // ДО mod: — modparam:... тоже начинается с "mod", но другой разделитель.
                let parts: Vec<&str> = v.splitn(2, ':').collect();
                if parts.len() != 2 {
                    return Action::Unknown;
                }
                let name = parts[0].to_string();
                let param = match parts[1] {
                    "keepalive" => crate::vpn::validate::ModifyParam::Keepalive,
                    "dns" => crate::vpn::validate::ModifyParam::Dns,
                    "allowedips" => crate::vpn::validate::ModifyParam::AllowedIps,
                    "endpoint" => crate::vpn::validate::ModifyParam::Endpoint,
                    _ => return Action::Unknown,
                };
                Action::ModifyParam(name, param)
            } else if let Some(v) = data.strip_prefix("mod:") {
                Action::Modify(v.to_string())
            } else {
                Action::Unknown
            }
        }
    }
}

type HandlerResult = Result<(), Box<dyn std::error::Error + Send + Sync>>;
type MyDialogue = Dialogue<State, InMemStorage<State>>;

fn user_id_of_msg(msg: &Message) -> Option<i64> {
    msg.from.as_ref().map(|u| u.id.0 as i64)
}

fn user_id_of_cb(q: &CallbackQuery) -> i64 {
    q.from.id.0 as i64
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn star_order_id(payload: &str) -> Option<i64> {
    payload.strip_prefix("awgram-stars:")?.parse().ok()
}

async fn send_star_invoice(
    bot: &Bot,
    chat: ChatId,
    order: &crate::store::StarOrder,
) -> HandlerResult {
    let action = if order.kind == "renew" {
        "Продление VPN"
    } else {
        "Подписка VPN"
    };
    bot.send_invoice(
        chat,
        format!("{action} на {} мес.", order.months),
        "Оплата цифровой услуги Telegram Stars. Доступ выдаётся автоматически после подтверждения платежа.",
        format!("awgram-stars:{}", order.id),
        "XTR",
        vec![LabeledPrice::new(action, order.stars as u32)],
    )
    .start_parameter(format!("stars_{}", order.id))
    .await?;
    Ok(())
}

fn tariff_duration(months: i64) -> Option<&'static str> {
    match months {
        1 => Some("30d"),
        3 => Some("90d"),
        6 => Some("180d"),
        12 => Some("365d"),
        _ => None,
    }
}

fn is_customer_navigation(text: &str) -> bool {
    text.starts_with("/start")
        || matches!(
            text,
            "🏠 Кабинет"
                | "🔑 Мои ключи"
                | "➕ Купить ключ"
                | "➕ Пополнить"
                | "📖 Инструкция"
                | "🆘 Поддержка"
                | "🎟 Промокод"
                | "♻️ Восстановить ключи"
        )
}

fn duration_seconds(value: &str) -> Option<i64> {
    let split = value.len().checked_sub(1)?;
    let amount = value[..split].parse::<i64>().ok()?;
    let unit = &value[split..];
    let multiplier = match unit {
        "h" => 3_600,
        "d" => 86_400,
        "w" => 604_800,
        "m" => 2_592_000,
        "y" => 31_536_000,
        _ => return None,
    };
    amount.checked_mul(multiplier).filter(|v| *v > 0)
}

fn customer_base_name(user: &crate::store::UserRow) -> String {
    let raw = user
        .username
        .clone()
        .unwrap_or_else(|| format!("user{}", user.user_id));
    let mut base: String = raw
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
        .take(28)
        .collect();
    if base.is_empty() || base.starts_with('-') {
        base = format!("user{}", user.user_id);
        base.truncate(28);
    }
    base
}

fn panel_secret(
    settings: &Store,
    server: &crate::store::VpnServer,
) -> crate::error::Result<String> {
    settings
        .panel_password(server.id)
        .ok_or_else(|| crate::error::Error::Parse("пароль панели не настроен".into()))
}

async fn nonlocal_add(
    vpn: &Vpn,
    settings: &Store,
    server: &crate::store::VpnServer,
    name: &str,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    if server.protocol == "amneziawg-panel" {
        vpn.panel_add(server, &panel_secret(settings, server)?, name)
            .await
    } else if let (Some(node), Some(secret)) = (
        settings.vpn_node_for_server(server.id),
        settings.node_secret(server.id),
    ) {
        vpn.agent_add(server, &node, &secret, name).await
    } else {
        vpn.remote_add(server, name).await
    }
}

async fn nonlocal_files(
    vpn: &Vpn,
    settings: &Store,
    server: &crate::store::VpnServer,
    name: &str,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    if server.protocol == "amneziawg-panel" {
        vpn.panel_existing_files(server, &panel_secret(settings, server)?, name)
            .await
    } else if let (Some(node), Some(secret)) = (
        settings.vpn_node_for_server(server.id),
        settings.node_secret(server.id),
    ) {
        vpn.agent_existing_files(server, &node, &secret, name).await
    } else {
        vpn.remote_existing_files(server, name).await
    }
}

async fn nonlocal_remove(
    vpn: &Vpn,
    settings: &Store,
    server: &crate::store::VpnServer,
    name: &str,
) -> crate::error::Result<()> {
    if server.protocol == "amneziawg-panel" {
        vpn.panel_remove(server, &panel_secret(settings, server)?, name)
            .await
    } else if let (Some(node), Some(secret)) = (
        settings.vpn_node_for_server(server.id),
        settings.node_secret(server.id),
    ) {
        vpn.agent_remove(server, &node, &secret, name).await
    } else {
        vpn.remote_remove(server, name).await
    }
}

async fn nonlocal_set_expiry(
    vpn: &Vpn,
    settings: &Store,
    server: &crate::store::VpnServer,
    name: &str,
    expires_at: i64,
) -> crate::error::Result<()> {
    if server.protocol == "amneziawg-panel" {
        vpn.panel_set_expiry(server, &panel_secret(settings, server)?, name, expires_at)
            .await
    } else if let (Some(node), Some(secret)) = (
        settings.vpn_node_for_server(server.id),
        settings.node_secret(server.id),
    ) {
        vpn.agent_set_expiry(server, &node, &secret, name, expires_at)
            .await
    } else {
        vpn.remote_set_expiry(server, name, expires_at).await
    }
}

async fn nonlocal_refresh(
    vpn: &Vpn,
    settings: &Store,
    server: &crate::store::VpnServer,
    name: &str,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    if server.protocol == "amneziawg-panel" {
        nonlocal_files(vpn, settings, server, name).await
    } else if let (Some(node), Some(secret)) = (
        settings.vpn_node_for_server(server.id),
        settings.node_secret(server.id),
    ) {
        vpn.agent_regen(server, &node, &secret, name).await
    } else {
        vpn.remote_regen(server, name).await
    }
}

async fn client_files(
    vpn: &Vpn,
    settings: &Store,
    name: &str,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    match settings.client_vpn_server(name) {
        Some(server) if !server.is_local => nonlocal_files(vpn, settings, &server, name).await,
        _ => vpn.existing_files(name),
    }
}

async fn client_remove(vpn: &Vpn, settings: &Store, name: &str) -> crate::error::Result<()> {
    let source = settings.client_vpn_server(name);
    if source
        .as_ref()
        .is_none_or(|server| server.status != "online")
    {
        settings.retire_client(name, now_epoch());
        return Ok(());
    }
    let result = match source {
        Some(server) if !server.is_local => nonlocal_remove(vpn, settings, &server, name).await,
        _ => vpn.remove(name).await,
    };
    if result.is_ok() || matches!(&result, Err(crate::error::Error::ClientNotFound(_))) {
        settings.retire_client(name, now_epoch());
        return Ok(());
    }
    result
}

async fn client_refresh(
    vpn: &Vpn,
    settings: &Store,
    name: &str,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    match settings.client_vpn_server(name) {
        Some(server) if !server.is_local => nonlocal_refresh(vpn, settings, &server, name).await,
        _ => vpn.regen_client(name).await,
    }
}

async fn managed_clients(
    vpn: &Vpn,
    settings: &Store,
) -> crate::error::Result<Vec<crate::vpn::model::Client>> {
    let mut clients = settings.registered_clients();
    if let Ok(local) = vpn.list_enriched().await {
        for client in local {
            if let Some(existing) = clients.iter_mut().find(|item| item.name == client.name) {
                *existing = client;
            } else {
                clients.push(client);
            }
        }
    }
    clients.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(clients)
}

fn legacy_recovery_server(settings: &Store) -> Option<crate::store::VpnServer> {
    let servers = settings.available_vpn_servers();
    let preferred = settings.default_vpn_server();
    preferred
        .and_then(|id| {
            servers
                .iter()
                .find(|server| server.id == id && !server.is_local)
        })
        .cloned()
        .or_else(|| {
            servers
                .iter()
                .find(|server| server.protocol == "amneziawg-panel" && !server.is_local)
                .cloned()
        })
        .or_else(|| servers.iter().find(|server| !server.is_local).cloned())
        .or_else(|| {
            preferred
                .and_then(|id| servers.iter().find(|server| server.id == id))
                .cloned()
        })
        .or_else(|| servers.into_iter().next())
}

async fn extend_managed_client(
    vpn: &Vpn,
    settings: &Store,
    name: &str,
    seconds: i64,
    now: i64,
) -> crate::error::Result<i64> {
    let target = vpn
        .client_expiry(name)
        .unwrap_or(now)
        .max(now)
        .saturating_add(seconds);
    set_managed_expiry(vpn, settings, name, target).await?;
    Ok(target)
}

async fn set_managed_expiry(
    vpn: &Vpn,
    settings: &Store,
    name: &str,
    target: i64,
) -> crate::error::Result<()> {
    match settings.client_vpn_server(name) {
        Some(server) if !server.is_local => {
            nonlocal_set_expiry(vpn, settings, &server, name, target).await?;
        }
        _ => {
            vpn.set_client_expiry(name, Some(target)).await?;
        }
    }
    Ok(())
}

async fn resume_pending_replacement(
    bot: &Bot,
    chat: ChatId,
    vpn: &Vpn,
    settings: &Store,
    lang: Lang,
    user_id: i64,
    old: &str,
) -> Result<bool, Box<dyn std::error::Error + Send + Sync>> {
    let Some((id, new, server_id)) = settings.pending_key_replacement(user_id, old) else {
        return Ok(false);
    };
    if settings.client_owner(&new) != Some(user_id) {
        // A previous attempt was rolled back physically but left a stale
        // pending row. Close it and let the caller perform one clean retry.
        settings.decide_key_replacement(id, user_id, "cancelled", now_epoch());
        return Ok(false);
    }
    settings.retire_client(old, now_epoch());
    let location = settings
        .vpn_server(server_id)
        .map(|server| server.location)
        .unwrap_or_else(|| "рабочий сервер".into());
    bot.send_message(chat, format!("♻️ Найдена уже созданная замена\n\nНовый ключ: «{new}»\nСервер: {location}\nСтарый ключ «{old}» скрыт из личного кабинета.\n\nУстановите новый ключ и подтвердите результат. Ещё один ключ создаваться не будет."))
        .reply_markup(menu::replacement_confirm_menu(id))
        .await?;
    match client_files(vpn, settings, &new).await {
        Ok(files) => render::send_client_files(bot, chat, lang, &files).await?,
        Err(error) => {
            bot.send_message(chat, i18n::error_text(lang, &error))
                .await?;
        }
    }
    Ok(true)
}

async fn provision_customer_key(
    vpn: &Vpn,
    settings: &Store,
    user_id: i64,
    months: i64,
    server_id: i64,
) -> crate::error::Result<crate::vpn::model::AddResult> {
    let server = settings
        .vpn_server(server_id)
        .ok_or_else(|| crate::error::Error::Parse("локация не найдена".to_string()))?;
    if !server.enabled_for_provisioning
        || settings.server_client_count(server_id) >= server.capacity
    {
        return Err(crate::error::Error::Parse(
            "на выбранной локации нет свободных мест".to_string(),
        ));
    }
    let user = settings
        .user(user_id)
        .ok_or_else(|| crate::error::Error::Parse("пользователь не зарегистрирован".to_string()))?;
    let existing = settings
        .active_client_names()
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let name = crate::vpn::validate::gen_available_names(&customer_base_name(&user), 1, &existing)
        .map_err(|e| crate::error::Error::Parse(e.to_string()))?
        .remove(0);
    let expiry = tariff_duration(months)
        .ok_or_else(|| crate::error::Error::Parse("неизвестный тариф".to_string()))?;
    let result = if server.is_local {
        vpn.add(&name, Some(expiry), settings.psk_default()).await?
    } else {
        let result = nonlocal_add(vpn, settings, &server, &name).await?;
        let seconds = duration_seconds(expiry)
            .ok_or_else(|| crate::error::Error::Parse("неверный срок тарифа".into()))?;
        if let Err(error) =
            nonlocal_set_expiry(vpn, settings, &server, &name, now_epoch() + seconds).await
        {
            tracing::error!(
                server_id = server.id,
                protocol = %server.protocol,
                client = %name,
                error = %error,
                "remote client expiry failed; rolling back newly created key"
            );
            let _ = nonlocal_remove(vpn, settings, &server, &name).await;
            return Err(error);
        }
        result
    };
    settings.assign_client_group(&name, None, now_epoch());
    settings.assign_client_owner(&name, Some(user_id));
    if !settings.assign_client_server(&name, server_id, &server.protocol) {
        if server.is_local {
            let _ = vpn.remove(&name).await;
        } else {
            let _ = nonlocal_remove(vpn, settings, &server, &name).await;
        }
        return Err(crate::error::Error::Parse(
            "не удалось закрепить ключ за сервером".to_string(),
        ));
    }
    Ok(result)
}

async fn customer_dashboard(
    bot: &Bot,
    chat: ChatId,
    uid: i64,
    vpn: &Vpn,
    settings: &Store,
) -> HandlerResult {
    let me = bot.get_me().await?;
    let username = me.username.clone().unwrap_or_default();
    let display_name = settings
        .user(uid)
        .map(|user| user.display_name)
        .unwrap_or_else(|| "пользователь".into());
    let now = now_epoch();
    let names = settings.user_client_names(uid);
    let working = names
        .iter()
        .filter(|name| {
            settings
                .client_vpn_server(name)
                .is_some_and(|server| server.status == "online")
                && vpn.client_expiry(name).is_none_or(|expiry| expiry > now)
        })
        .count();
    let broken = names
        .iter()
        .filter(|name| {
            settings
                .client_vpn_server(name)
                .is_none_or(|server| server.status != "online")
        })
        .count();
    let expiring = names
        .iter()
        .filter(|name| {
            vpn.client_expiry(name)
                .is_some_and(|expiry| expiry > now && expiry <= now + 7 * 86_400)
        })
        .count();
    bot.send_message(
        chat,
        format!(
            "🏠 Личный кабинет\nЗдравствуйте, {display_name}!\n\n🔐 Подключения\n✅ Работают: {working}\n❌ Требуют замены: {broken}\n⏳ Истекают за 7 дней: {expiring}\n\n💰 Баланс: {:.2} ₽\n👥 Приглашено друзей: {}\n\nБыстрый старт:\n• «🔑 Мои ключи» — подключение и замена\n• «➕ Купить ключ» — новое устройство\n• «🆘 Поддержка» — помощь с подключением\n\n🔗 Реферальная ссылка:\nhttps://t.me/{username}?start=ref_{uid}",
            settings.balance_kopecks(uid) as f64 / 100.0,
            settings.referral_count(uid),
        ),
    )
    .reply_markup(menu::customer_keyboard())
    .await?;
    Ok(())
}

fn customer_key_list(
    settings: &Store,
    vpn: &Vpn,
    uid: i64,
) -> (Vec<String>, Vec<(String, String)>) {
    let names = settings.user_client_names(uid);
    let mut lines = Vec::with_capacity(names.len());
    let mut buttons = Vec::with_capacity(names.len());
    for name in names {
        let device = settings
            .device_label(&name)
            .unwrap_or_else(|| "устройство не указано".into());
        let server = settings.client_vpn_server(&name);
        let unavailable = server
            .as_ref()
            .is_none_or(|server| server.status != "online");
        let (icon, state) = if unavailable {
            ("❌", "НЕ РАБОТАЕТ — требуется замена".to_string())
        } else if vpn.client_disabled(&name) {
            ("⏸", "отключён".to_string())
        } else {
            ("✅", "работает".to_string())
        };
        let location = server
            .as_ref()
            .map(|server| server.location.as_str())
            .unwrap_or("сервер не определён");
        let expiry = crate::vpn::model::format_expiry(
            settings.lang(uid),
            now_epoch(),
            vpn.client_expiry(&name),
        );
        lines.push(format!(
            "{icon} {device}\n   Ключ: {name}\n   Сервер: {location}\n   Статус: {state}\n   Срок: {expiry}"
        ));
        let title = format!("{icon} {device} · {name}")
            .chars()
            .take(60)
            .collect::<String>();
        buttons.push((name.clone(), title));
    }
    (lines, buttons)
}

async fn admin_dashboard(bot: &Bot, chat: ChatId, vpn: &Vpn, settings: &Store) -> HandlerResult {
    let now = now_epoch();
    let clients = vpn.list().await.unwrap_or_default();
    let disabled = clients
        .iter()
        .filter(|c| vpn.client_disabled(&c.name))
        .count();
    let expiring = clients
        .iter()
        .filter(|c| {
            vpn.client_expiry(&c.name)
                .is_some_and(|e| e > now && e - now <= 7 * 86_400)
        })
        .count();
    let month = settings.finance_summary(now - 30 * 86_400);
    let servers = settings.vpn_servers();
    let online_servers = servers
        .iter()
        .filter(|server| server.status == "online")
        .count();
    let total_keys = settings.active_client_names().len();
    bot.send_message(chat,format!(
        "🏠 Панель управления ZuevVPN\n\n🖥 Инфраструктура\nСерверы: {online_servers}/{} онлайн\nКлючи: {total_keys} активных\n\n👥 Клиенты\nПользователей: {}\nОтключено: {disabled}\nИстекают за 7 дней: {expiring}\n\n💼 Работа\nПлатежей ожидает: {}\nОбращений открыто: {}\nВыручка за 30 дней: {:.2} ₽\n\n⚙️ Версия бота: v{}\n\nВыберите раздел:",
        servers.len(),settings.all_user_ids().len(),month.pending,settings.open_support_count(),month.revenue_kopecks as f64/100.0,env!("CARGO_PKG_VERSION")))
        .reply_markup(menu::admin_dashboard_menu()).await?;
    Ok(())
}

async fn owners_screen(
    bot: &Bot,
    chat: ChatId,
    settings: &Store,
    requested_page: usize,
) -> HandlerResult {
    const PAGE_SIZE: usize = 10;
    let all = settings
        .all_user_ids()
        .into_iter()
        .filter_map(|id| settings.user(id))
        .rev()
        .collect::<Vec<_>>();
    let pages = all.len().max(1).div_ceil(PAGE_SIZE);
    let page = requested_page.min(pages - 1);
    let users = all
        .iter()
        .skip(page * PAGE_SIZE)
        .take(PAGE_SIZE)
        .cloned()
        .collect::<Vec<_>>();
    bot.send_message(
        chat,
        format!(
            "👤 Владельцы ключей\n\nВсего пользователей: {}\nСтраница {} из {} · по {PAGE_SIZE} записей.\nОткройте карточку или воспользуйтесь поиском.",
            all.len(), page + 1, pages
        ),
    )
    .reply_markup(menu::admin_users_menu(&users, page, pages))
    .await?;
    Ok(())
}

async fn admin_user_screen(
    bot: &Bot,
    chat: ChatId,
    settings: &Store,
    user_id: i64,
) -> HandlerResult {
    let Some(p) = settings.admin_user_profile(user_id) else {
        bot.send_message(chat, "Пользователь не найден.")
            .reply_markup(menu::admin_users_menu(&[], 0, 1))
            .await?;
        return Ok(());
    };
    let username = p
        .user
        .username
        .as_ref()
        .map(|v| format!("@{v}"))
        .unwrap_or_else(|| "—".into());
    let note = p.admin_note.as_deref().unwrap_or("—");
    bot.send_message(chat,format!(
        "👤 Карточка пользователя\n\n{}\nTelegram ID: {}\nUsername: {}\nСтатус: {}\nСоздан: {}\nПоследняя активность: {}\n\n💰 Баланс: {:.2} ₽\n💳 Платежей: {} · Потрачено: {:.2} ₽\n🔑 Ключей: {}\n👥 Рефералов: {}\n🆘 Обращений: {}\n📝 Заметка: {}",
        p.user.display_name,p.user.user_id,username,if p.blocked{"⛔ заблокирован"}else{"✅ активен"},p.user.created_at,p.last_seen,p.balance_kopecks as f64/100.0,p.payment_count,p.spent_kopecks as f64/100.0,p.key_count,p.referral_count,p.ticket_count,note
    )).reply_markup(menu::admin_user_menu(user_id,p.blocked)).await?;
    Ok(())
}

async fn support_screen(bot: &Bot, chat: ChatId, settings: &Store) -> HandlerResult {
    let mut tickets = settings.support_tickets("open", 25);
    tickets.extend(settings.support_tickets("in_progress", 25));
    bot.send_message(
        chat,
        format!("🆘 Поддержка\nАктивных обращений: {}", tickets.len()),
    )
    .reply_markup(menu::support_filters_menu(&tickets))
    .await?;
    Ok(())
}

async fn support_filtered_screen(
    bot: &Bot,
    chat: ChatId,
    settings: &Store,
    status: &str,
) -> HandlerResult {
    let tickets = settings.support_tickets(status, 50);
    bot.send_message(
        chat,
        format!("🆘 Обращения · {status}\nНайдено: {}", tickets.len()),
    )
    .reply_markup(menu::support_filters_menu(&tickets))
    .await?;
    Ok(())
}

async fn finance_screen(bot: &Bot, chat: ChatId, settings: &Store) -> HandlerResult {
    let now = now_epoch();
    let day = settings.finance_summary(now - 86_400);
    let month = settings.finance_summary(now - 30 * 86_400);
    let pending = settings.pending_payments();
    bot.send_message(chat,format!("💳 Финансы\n\nСегодня: {} продаж · {:.2} ₽\n30 дней: {} продаж · {:.2} ₽\nПополнения: {:.2} ₽\nВозвраты: {:.2} ₽\nОжидают решения: {}",day.approved_sales,day.revenue_kopecks as f64/100.0,month.approved_sales,month.revenue_kopecks as f64/100.0,month.topups_kopecks as f64/100.0,month.refunds_kopecks as f64/100.0,month.pending)).reply_markup(menu::finance_dashboard_menu(&pending)).await?;
    Ok(())
}

async fn legacy_admin_screen(bot: &Bot, chat: ChatId, settings: &Store) -> HandlerResult {
    let requests = settings.legacy_requests("pending", 100);
    let price = settings.legacy_renewal_price_kopecks();
    let details = requests
        .iter()
        .map(|r| {
            let user = settings.user(r.user_id);
            let username = user
                .and_then(|u| u.username.map(|v| format!("@{v}")))
                .unwrap_or_else(|| "без username".into());
            format!(
                "#{} · {} · ID {} · имя: {} · {}",
                r.id,
                username,
                r.user_id,
                r.requested_name,
                r.comment.as_deref().unwrap_or("без комментария")
            )
        })
        .collect::<Vec<_>>();
    bot.send_message(chat,format!("♻️ Legacy-ключи\n\nОжидают проверки: {}\nБазовая цена ежегодного продления: {:.2} ₽\n6–9 legacy-ключей: 750 ₽ за ключ/год\n10 и более: 500 ₽ за ключ/год\nПриём заявок закрывается 01.12.2026.\n\n{}",requests.len(),price as f64/100.0,if details.is_empty(){"Новых заявок нет".into()}else{details.join("\n")})).reply_markup(menu::legacy_admin_menu(&requests)).await?;
    Ok(())
}

fn server_card_text(server: &crate::store::VpnServer, now: i64) -> String {
    let opened = server
        .opened_at
        .map(crate::calendar::format_date)
        .unwrap_or_else(|| "не указано".into());
    let paid = server
        .paid_until
        .map(crate::calendar::format_date)
        .unwrap_or_else(|| "не настроено".into());
    let cost = server
        .cost_minor
        .map(|v| {
            format!(
                "{:.2} {}",
                v as f64 / 100.0,
                server.currency.as_deref().unwrap_or("")
            )
        })
        .unwrap_or_else(|| "не указана".into());
    let days = server
        .paid_until
        .map(|v| (v - now).div_euclid(86_400))
        .map(|v| format!("{v} дн."))
        .unwrap_or_else(|| "—".into());
    format!("🖥 {}\n\n📡 Состояние\nСтатус: {}\nРоль: {}\nВыдача ключей: {}\nПротокол: {}\nЛимит: {} ключей\n\n🌍 Подключение\nЛокация: {}\nIP: {}\nHostname: {}\nПровайдер: {}\n\n💳 Оплата VPS\nОплачен до: {} ({})\nСтоимость: {} / {} мес.\nАвтопродление: {}\n\n🗂 Учёт\nОткрыт: {}\nДобавлен в бот: {}",
        server.name,server.status,if server.is_local{"🏠 локальный сервер бота"}else{"☁️ удалённый VPN-сервер"},if server.enabled_for_provisioning{"включена"}else{"выключена"},if server.protocol=="amneziawg-2"{"AWG 2.0"}else{"AWG 1.0"},server.capacity,server.location,server.public_ip,server.hostname,server.provider,paid,days,cost,server.billing_period_months.map(|v|v.to_string()).unwrap_or_else(||"—".into()),if server.auto_renew{"да"}else{"нет"},opened,crate::calendar::format_date(server.added_at))
}

async fn servers_screen(bot: &Bot, chat: ChatId, settings: &Store) -> HandlerResult {
    let servers = settings.vpn_servers();
    let attention = servers
        .iter()
        .filter(|s| {
            matches!(s.status.as_str(), "warning" | "offline")
                || s.paid_until.is_some_and(|v| v <= now_epoch() + 7 * 86_400)
        })
        .count();
    let local = servers.iter().filter(|s| s.is_local).count();
    let online = servers.iter().filter(|s| s.status == "online").count();
    bot.send_message(chat,format!("🖥 Серверы\n\n🟢 Онлайн: {online}/{}\n🏠 Локальных: {local}\n⚠️ Требуют внимания: {attention}\n\nВыберите конкретный сервер. В его карточке действия разделены на подключение, диагностику, ключи и обслуживание.",servers.len())).reply_markup(menu::servers_menu(&servers)).await?;
    Ok(())
}

fn parse_minor(value: &str) -> Option<i64> {
    let normalized = value.trim().replace(',', ".");
    let mut parts = normalized.split('.');
    let whole = parts.next()?.parse::<i64>().ok()?;
    let fraction = parts.next().unwrap_or("0");
    if parts.next().is_some() || fraction.len() > 2 || whole < 0 {
        return None;
    }
    let cents = match fraction.len() {
        0 => 0,
        1 => fraction.parse::<i64>().ok()? * 10,
        _ => fraction.parse().ok()?,
    };
    whole.checked_mul(100)?.checked_add(cents)
}

async fn maybe_issue_trial(bot: &Bot, chat: ChatId, vpn: &Vpn, settings: &Store, uid: i64) {
    let claimed_at = now_epoch();
    if !settings.claim_trial(uid, claimed_at) {
        return;
    }
    let Some(user) = settings.user(uid) else {
        settings.release_trial_claim(uid, claimed_at);
        return;
    };
    let result = async {
        let server_id = settings.default_vpn_server().ok_or_else(|| {
            crate::error::Error::Parse("основной сервер выдачи не настроен".into())
        })?;
        let server = settings
            .available_vpn_servers()
            .into_iter()
            .find(|server| server.id == server_id)
            .ok_or_else(|| {
                crate::error::Error::Parse("основной сервер выдачи недоступен".into())
            })?;
        let existing = settings.active_client_names().into_iter().collect();
        let name = crate::vpn::validate::gen_available_names(
            &format!("{}_trial", customer_base_name(&user)),
            1,
            &existing,
        )
        .map_err(|e| crate::error::Error::Parse(e.to_string()))?
        .remove(0);
        let result = if server.is_local {
            vpn.add(&name, Some("1d"), settings.psk_default()).await?
        } else {
            let result = nonlocal_add(vpn, settings, &server, &name).await?;
            if let Err(error) =
                nonlocal_set_expiry(vpn, settings, &server, &name, claimed_at + 86_400).await
            {
                let _ = nonlocal_remove(vpn, settings, &server, &name).await;
                return Err(error);
            }
            result
        };
        settings.assign_client_group(&name, None, claimed_at);
        settings.assign_client_owner(&name, Some(uid));
        if !settings.assign_client_server(&name, server.id, &server.protocol) {
            if server.is_local {
                let _ = vpn.remove(&name).await;
            } else {
                let _ = nonlocal_remove(vpn, settings, &server, &name).await;
            }
            return Err(crate::error::Error::Parse(
                "не удалось закрепить пробный ключ за сервером".into(),
            ));
        }
        Ok::<_, crate::error::Error>(result)
    }
    .await;
    match result {
        Ok(result) => {
            let _ = bot
                .send_message(chat, "🎁 Вам выдан бесплатный тестовый ключ на 24 часа.")
                .await;
            let _ = render::send_client_files(bot, chat, settings.lang(uid), &result).await;
        }
        Err(error) => {
            settings.release_trial_claim(uid, claimed_at);
            tracing::error!(%error, user_id = uid, "не удалось выдать пробный ключ");
            let _ = bot
                .send_message(
                    chat,
                    "Не удалось автоматически выдать тестовый ключ. Напишите в поддержку.",
                )
                .await;
        }
    }
}

/// Обрезает вывод скрипта до лимита Telegram-сообщения (3500 байт, с запасом
/// на HTML-обёртку), округляя вниз до границы UTF-8-символа — байтовый индекс
/// может попасть внутрь многобайтового символа (кириллица в выводе скрипта).
fn truncate_for_message(body: String) -> String {
    if body.len() <= 3500 {
        return body;
    }
    let mut cut = 3500;
    while !body.is_char_boundary(cut) {
        cut -= 1;
    }
    format!("{}\n…", &body[..cut])
}

async fn latest_release_info() -> Option<(String, String, String)> {
    let response = reqwest::Client::builder()
        .user_agent("awgram-update-check")
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?
        .get("https://api.github.com/repos/stevefoxru/awgram/releases/latest")
        .send()
        .await
        .ok()?
        .error_for_status()
        .ok()?
        .json::<serde_json::Value>()
        .await
        .ok()?;
    let tag = response.get("tag_name")?.as_str()?.to_string();
    let body = response
        .get("body")
        .and_then(|value| value.as_str())
        .unwrap_or("Список изменений не опубликован.")
        .chars()
        .take(1800)
        .collect::<String>();
    let url = response
        .get("html_url")
        .and_then(|value| value.as_str())
        .unwrap_or("https://github.com/stevefoxru/awgram/releases/latest")
        .to_string();
    Some((tag, body, url))
}

/// Локальный текст сессии-таймаута: не входит в каталог `i18n` (см. brief
/// задачи 5 — новые фичи в других задачах), но всё равно локализуется, чтобы
/// не оставлять непереведённых строк в слое `bot/`.
fn session_expired_text(lang: Lang) -> &'static str {
    match lang {
        Lang::Ru => "Сессия устарела. Начните заново.",
        Lang::En => "Session expired. Start again.",
    }
}

fn unknown_action_text(lang: Lang) -> &'static str {
    match lang {
        Lang::Ru => "Неизвестное действие.",
        Lang::En => "Unknown action.",
    }
}

/// Рендер экрана навигации (меню / список клиентов / страница списка)
/// редактированием сообщения, на кнопке которого нажали (`msg_id` — это
/// `q.message`). Применяется в `Action::Menu`, `Action::List`, `Action::Page`:
/// меню↔список↔пагинация эволюционируют в одном сообщении — без спама и без
/// глобального HashMap с message_id (его предлагал issue #16, но источник
/// редактируемого сообщения у нас уже есть — это само `q.message`).
///
/// Поведение при ошибках:
/// · `MessageNotModified` (контент не изменился — напр. 🔄 без изменений)
///   — глотаем, это успешный no-op;
/// · любая иная ошибка (сообщение удалено/не текст/устарело) — откат к
///   `send_message`, а со старого сообщения снимается inline-клавиатура
///   (пустой markup), чтобы в чате не висели две живых клавиатуры. Если
///   старое уже удалено — `edit_message_reply_markup` тоже упадёт, ошибку
///   игнорируем.
async fn edit_or_send(
    bot: &Bot,
    chat: ChatId,
    msg_id: MessageId,
    text: String,
    kb: InlineKeyboardMarkup,
) {
    let edit = bot
        .edit_message_text(chat, msg_id, text.clone())
        .reply_markup(kb.clone())
        .parse_mode(ParseMode::Html)
        .await;
    if let Err(e) = edit {
        match e {
            teloxide::errors::RequestError::Api(teloxide::errors::ApiError::MessageNotModified) => {
                // Контент не изменился (нажали 🔄 без изменений) — норма.
            }
            e => {
                tracing::debug!(error = %e, "edit_message_text не удался — отправляю новое");
                // Снимаем клавиатуру со старого сообщения — ниже уйдёт новое с
                // живой клавиатурой, и двух активных рядом быть не должно.
                let _ = bot
                    .edit_message_reply_markup(chat, msg_id)
                    .reply_markup(InlineKeyboardMarkup::default())
                    .await;
                let _ = bot
                    .send_message(chat, text)
                    .reply_markup(kb)
                    .parse_mode(ParseMode::Html)
                    .await;
            }
        }
    }
}

/// Домашняя клавиатура по роли: владельцу — полное меню, групповому админу —
/// его сокращённое (кнопка смены группы при нескольких группах).
pub fn home_menu(role: &Role, lang: Lang) -> InlineKeyboardMarkup {
    match role {
        Role::GroupAdmin(groups) => menu::ga_main_menu(lang, groups.len() > 1),
        Role::Owner => menu::admin_dashboard_menu(),
        Role::Staff(_) => menu::main_menu(lang),
        _ => menu::main_menu(lang),
    }
}

/// Рабочая группа группового админа: единственная — сразу она; из нескольких —
/// сохранённый выбор, если он всё ещё валиден; иначе None (нужен экран выбора).
pub fn current_ga_group(settings: &Store, uid: i64, groups: &[i64]) -> Option<i64> {
    match groups {
        [only] => Some(*only),
        _ => settings.current_group(uid).filter(|g| groups.contains(g)),
    }
}

/// Клиент в зоне видимости роли? Владелец видит всех; групповой админ — только
/// клиентов своих групп (без группы — не видны). Логика в Role::can_see_client.
fn client_in_scope(role: &Role, settings: &Store, name: &str) -> bool {
    role.can_see_client(settings.client_group(name))
}

/// Единая точка авторизации callback-Action. Исчерпывающий match БЕЗ
/// wildcard — новый вариант Action не скомпилируется, пока автор явно не
/// отнесёт его к классу доступа (раньше guard'ы были размазаны по веткам
/// диспатча, и забытый guard в новом Action ничем не ловился).
fn authorize(action: &Action, role: &Role, settings: &Store) -> bool {
    use Action::*;
    if *role == Role::Denied {
        return false;
    }
    if let Role::Staff(staff_role) = role {
        return match staff_role.as_str() {
            "support" => matches!(
                action,
                AdminSupport
                    | SupportFilter(_)
                    | SupportTicket(_)
                    | SupportTake(_)
                    | SupportReply(_)
                    | SupportClose(_)
                    | SupportPriority(_, _)
                    | Menu
            ),
            "finance" => matches!(
                action,
                AdminFinance | FinanceExport | PaymentApprove(_) | PaymentReject(_) | Menu
            ),
            "technical" => matches!(
                action,
                Menu | List
                    | Stats
                    | Page(_)
                    | ShowClient(_)
                    | ClientHistory(_)
                    | SendConf(_)
                    | SendQr(_)
                    | SendLink(_)
                    | SendAll(_)
                    | Modify(_)
                    | ModifyParam(_, _)
                    | Regen(_)
                    | SetClientEnabled(_, _)
            ),
            _ => false,
        };
    }
    match action {
        // Доступно всем аутентифицированным ролям.
        Menu
        | List
        | Add
        | Stats
        | Page(_)
        | Expiry(_)
        | AddPsk(_)
        | AddServer(_)
        | AddBulk
        | AddBulkRun(_)
        | BulkExpiry(_)
        | AddBulkPsk(_)
        | BulkServer(_)
        | Lang(_)
        | SetListFilter(_)
        | Buy
        | BuyServer(_)
        | BuyTerm(_)
        | BuyMethod(_, _)
        | BuyPaid(_)
        | MyKeys
        | Profile
        | Portal
        | Balance
        | CustomerKey(_)
        | CustomerMove(_)
        | CustomerMoveServer(_, _)
        | CustomerMoveConfirm(_)
        | CustomerMoveCancel(_)
        | CustomerRefresh(_)
        | CustomerRefreshRun(_)
        | Renew(_)
        | RenewTerm(_, _)
        | RenewMethod(_, _, _)
        | LegacyRenew(_)
        | LegacyRenewMethod(_, _)
        | LegacyRequestNew
        | PromoInput
        | Guide(_)
        | AutoRenew(_, _, _)
        | DeviceLabelAsk(_)
        | SupportRate(_, _)
        | Unknown => true,
        // Экран/установка текущей группы: только GA, группа — только своя.
        GroupSelectMenu => matches!(role, Role::GroupAdmin(_)),
        GroupSelect(id) => matches!(role, Role::GroupAdmin(groups) if groups.contains(id)),
        // Действия над конкретным клиентом — по скоупу роли.
        ShowClient(name) | ClientHistory(name) | SendConf(name) | SendQr(name) | SendLink(name)
        | SendAll(name) | AskDelete(name) | ConfirmDelete(name) | Recreate(name) | Regen(name) => {
            client_in_scope(role, settings, name)
        }
        // Всё остальное — только владелец.
        RegenAll
        | RegenAllRun(_)
        | Settings
        | SetLang(_)
        | SetPsk(_)
        | SetSlug(_)
        | SetConf(_)
        | SetQr(_)
        | SetLink(_)
        | Modify(_)
        | ModifyParam(_, _)
        | Restart
        | RestartRun
        | RepairModule
        | Backup
        | BackupNew
        | BackupList
        | BackupCard(_)
        | BackupDownload(_)
        | Restore(_)
        | RestoreYes(_)
        | Check
        | Diagnose
        | Groups
        | GroupCreate
        | GroupCard(_)
        | GroupRenameAsk(_)
        | GroupQuotaAsk(_)
        | GroupAdmins(_)
        | GroupAdminRemove(_, _)
        | GroupInvite(_)
        | GroupInviteRevoke(_)
        | GroupAdminById(_)
        | GroupDeleteAsk(_)
        | GroupDeleteDetach(_)
        | GroupDeleteAllAsk(_)
        | GroupDeleteAllYes(_)
        | GroupRegenAsk(_)
        | GroupRegenRun(_)
        | MoveClientAsk(_)
        | MoveClientTo(_, _)
        | GroupScopeAsk
        | GroupScopeSet(_)
        | PaymentApprove(_)
        | PaymentReject(_)
        | AssignOwnerAsk(_)
        | AdminExpiryAsk(_)
        | SetClientEnabled(_, _)
        | SupportTicket(_)
        | SupportNewCategory(_)
        | SupportTake(_)
        | SupportReply(_)
        | SupportClose(_)
        | SupportPriority(_, _)
        | FinanceExport
        | AdminDashboard
        | AdminVpn
        | AdminServers
        | AdminKeys
        | AdminUsersHub
        | AdminCommunication
        | AdminSystem
        | AdminUpdate
        | AdminUpdateRun
        | AdminUpdateStatus
        | AdminUpdateRollback
        | ServerAdd
        | ServerCard(_)
        | RemoteMigration(_)
        | RemoteMigrationPreflight(_)
        | RemoteMigrationStatus(_)
        | RemoteMigrationTest(_)
        | RemoteMigrationApprove(_)
        | RemoteMigrationAsk(_)
        | RemoteMigrationRun(_)
        | RemoteMigrationRollback(_)
        | ServerBilling
        | ServerBillingAsk(_)
        | ServerPassportAsk(_)
        | ServerEnroll(_)
        | ServerEnrollRevoke(_)
        | ServerSetDefault(_)
        | ServerDeployAsk(_)
        | ServerCheck(_)
        | ServerDiagnose(_)
        | ServerProvisioningProbe(_)
        | ServerPanelConnect(_)
        | ServerPanelSync(_)
        | ServerPanelAudit(_)
        | LocalMigration
        | LocalMigrationPreflight
        | LocalMigrationStart
        | LocalMigrationStatus
        | LocalMigrationRollback
        | AdminCreate
        | AdminOwners
        | AdminOwnersPage(_)
        | AdminFinance
        | AdminSupport
        | AdminBroadcast
        | AdminBroadcastTemplates
        | BroadcastAudience(_)
        | BroadcastRetry(_)
        | AdminHelp
        | AdminSearch
        | AdminRoles
        | AdminRoleAction(_)
        | AdminBulk(_)
        | AdminBulkConfirm
        | AdminUser(_)
        | AdminUserKeys(_)
        | AdminUserDeleteKeysAsk(_)
        | AdminUserDeleteKeysConfirm(_)
        | AdminUserPayments(_)
        | AdminUserBalance(_)
        | AdminUserDiscount(_)
        | AdminUserNote(_)
        | AdminUserBlock(_, _)
        | StatsSection(_)
        | SupportFilter(_)
        | AdminPromos
        | AdminCommerce
        | AdminPricesRub
        | AdminPricesStars
        | AdminReferral
        | AdminPromoAction(_)
        | LegacyRestore
        | LegacyRequestApprove(_)
        | LegacyRequestReject(_)
        | LegacyPriceAsk
        | ClientNoteAsk(_)
        | PaymentInstructionsAsk
        | AcquiringUrlAsk => role.is_owner(),
    }
}

/// Группа для привязки клиента в finish_add. При recreate — существующая
/// привязка: пересоздание не отвязывает клиента у владельца и не переносит
/// его в текущую группу группового админа (скоуп на объект уже перепроверен
/// вызывающим). Новому клиенту: групповому админу — его текущая группа,
/// владельцу — выбранная в фильтре группа (если выбрана), иначе без группы.
/// None — текущей группы нет (нужен экран выбора).
fn group_for_new_client(
    role: &Role,
    settings: &Store,
    uid: i64,
    recreate: bool,
    name: &str,
) -> Option<Option<i64>> {
    if recreate {
        return Some(settings.client_group(name));
    }
    match role {
        Role::GroupAdmin(groups) => current_ga_group(settings, uid, groups).map(Some),
        Role::Owner => Some(match settings.owner_scope(uid) {
            ListScope::Group(id) => Some(id),
            _ => None,
        }),
        Role::Denied => Some(None),
        Role::Staff(_) => Some(None),
    }
}

/// Нужен ли откат создания: не-recreate клиент не влез в квоту группы
/// (проиграна гонка — ранняя проверка прошла, атомарная привязка нет).
fn add_needs_quota_rollback(recreate: bool, outcome: &crate::store::QuotaAssign) -> bool {
    !recreate && *outcome == crate::store::QuotaAssign::Full
}

/// Скоуп по роли: владельцу — сохранённый фильтр группы; групповому админу —
/// текущая группа или None (нужен экран выбора группы).
fn scope_for(role: &Role, settings: &Store, uid: i64) -> Option<ListScope> {
    match role {
        Role::GroupAdmin(groups) => current_ga_group(settings, uid, groups).map(ListScope::Group),
        Role::Staff(_) => Some(ListScope::All),
        _ => Some(settings.owner_scope(uid)),
    }
}

/// Экран выбора группы для группового админа (общий для List/Stats/Menu/Add).
async fn show_group_select(
    bot: &Bot,
    chat: ChatId,
    msg_id: MessageId,
    lang: Lang,
    settings: &Store,
    groups: &[i64],
) {
    let rows: Vec<_> = groups.iter().filter_map(|g| settings.group(*g)).collect();
    edit_or_send(
        bot,
        chat,
        msg_id,
        i18n::select_group_title(lang),
        menu::group_select_menu(lang, &rows),
    )
    .await;
}

/// Перерисовывает экран настроек: заголовок и клавиатура собираются из одних
/// и тех же текущих значений тумблеров (единственное место, где они читаются
/// для этого экрана).
async fn show_settings(bot: &Bot, chat: ChatId, msg_id: MessageId, lang: Lang, settings: &Store) {
    edit_or_send(
        bot,
        chat,
        msg_id,
        i18n::settings_title(
            lang,
            settings.psk_default(),
            settings.name_slug(),
            settings.deliver_conf(),
            settings.deliver_qr(),
            settings.deliver_link(),
        ),
        menu::settings_menu(
            lang,
            settings.psk_default(),
            settings.name_slug(),
            settings.deliver_conf(),
            settings.deliver_qr(),
            settings.deliver_link(),
        ),
    )
    .await;
}

/// Перерисовывает карточку группы: заголовок и клавиатура из одного чтения БД.
async fn show_group_card(
    bot: &Bot,
    chat: ChatId,
    msg_id: MessageId,
    lang: Lang,
    settings: &Store,
    id: i64,
) {
    let Some(g) = settings.group(id) else {
        let _ = bot.send_message(chat, i18n::not_found(lang)).await;
        return;
    };
    let count = settings.group_client_count(id);
    let admins = settings.group_admin_ids(id);
    let has_invite = settings.active_invite(id, now_epoch()).is_some();
    edit_or_send(
        bot,
        chat,
        msg_id,
        i18n::group_card(lang, &g.name, count, g.max_clients, admins.len()),
        menu::group_card_menu(lang, id, has_invite),
    )
    .await;
}

async fn message_handler(
    bot: Bot,
    dialogue: MyDialogue,
    msg: Message,
    cfg: Arc<Config>,
    vpn: Arc<Vpn>,
    settings: Arc<Store>,
) -> HandlerResult {
    if !msg.chat.is_private() {
        // Бот доставляет секреты (конфиги, QR, ссылки, бэкапы, диагностику) в чат
        // апдейта, а авторизует по user_id — в группе это грозит утечкой всем
        // участникам. Отклоняем до auth-гейта, чтобы вообще не трогать VPN/settings.
        bot.send_message(msg.chat.id, i18n::private_only()).await?;
        return Ok(());
    }

    let uid = user_id_of_msg(&msg).unwrap_or(0);
    if let Some(user) = msg.from.as_ref() {
        let referrer = msg
            .text()
            .and_then(|t| t.strip_prefix("/start ref_"))
            .and_then(|v| v.trim().parse::<i64>().ok())
            .filter(|id| settings.user(*id).is_some());
        settings.upsert_user(
            uid,
            user.username.as_deref(),
            &user.full_name(),
            referrer,
            now_epoch(),
        );
    }

    if let Some(payment) = msg.successful_payment() {
        let Some(order_id) = star_order_id(&payment.invoice_payload) else {
            bot.send_message(
                msg.chat.id,
                "Платёж получен, но заказ не распознан. Обратитесь в поддержку: /paysupport",
            )
            .await?;
            return Ok(());
        };
        let claim = settings.claim_star_payment(
            order_id,
            uid,
            i64::from(payment.total_amount),
            &payment.telegram_payment_charge_id.0,
            now_epoch(),
        );
        let crate::store::StarPaymentClaim::New(order) = claim else {
            if matches!(claim, crate::store::StarPaymentClaim::Invalid) {
                bot.send_message(
                    msg.chat.id,
                    "Не удалось подтвердить параметры платежа. Напишите в поддержку: /paysupport",
                )
                .await?;
            }
            return Ok(());
        };
        let result = if order.kind == "purchase" {
            match order.server_id {
                Some(server_id) => {
                    provision_customer_key(&vpn, &settings, uid, order.months, server_id)
                        .await
                        .map(|created| {
                            (
                                format!("✅ Оплата получена. Ключ «{}» создан.", created.name),
                                Some(created),
                            )
                        })
                }
                None => Err(crate::error::Error::Parse(
                    "локация заказа не указана".into(),
                )),
            }
        } else {
            let name = order.client_name.clone().unwrap_or_default();
            let seconds = tariff_duration(order.months)
                .and_then(duration_seconds)
                .unwrap_or_default();
            extend_managed_client(&vpn, &settings, &name, seconds, now_epoch())
                .await
                .map(|_| {
                    (
                        format!(
                            "✅ Оплата получена. Ключ «{name}» продлён на {} мес.",
                            order.months
                        ),
                        None,
                    )
                })
        };
        match result {
            Ok((text, created)) => {
                settings.finish_star_order(order.id, None, now_epoch());
                bot.send_message(msg.chat.id, text)
                    .reply_markup(menu::customer_keyboard())
                    .await?;
                if let Some(created) = created {
                    render::send_client_files(&bot, msg.chat.id, settings.lang(uid), &created)
                        .await?;
                }
            }
            Err(error) => {
                settings.finish_star_order(order.id, Some(&error.to_string()), now_epoch());
                bot.send_message(msg.chat.id, "Оплата подтверждена, но автоматическая выдача не завершилась. Администратор уже получил уведомление; сохраните сообщение и обратитесь в /paysupport.").await?;
                for owner in &cfg.admin_ids {
                    let _ = bot
                        .send_message(
                            ChatId(*owner),
                            format!(
                                "🚨 Stars-заказ #{} оплачен, но не исполнен: {error}",
                                order.id
                            ),
                        )
                        .await;
                }
            }
        }
        return Ok(());
    }

    // Инвайт-ссылка: /start inv_<token>. Обрабатывается ДО роль-гейта —
    // приглашённый ещё не имеет никакой роли. Токен одноразовый с TTL, так что
    // подбор мусорных токенов даёт лишь "ссылка недействительна".
    if let Some(payload) = msg
        .text()
        .and_then(|t| t.strip_prefix("/start inv_"))
        .map(str::trim)
    {
        let lang = settings.lang(uid);
        match settings.use_invite(payload, uid, now_epoch()) {
            crate::store::InviteUse::Joined(gid) => {
                let gname = settings.group(gid).map(|g| g.name).unwrap_or_default();
                settings.log_event(
                    now_epoch(),
                    EventKind::InviteUse,
                    None,
                    Some(uid),
                    Some(&format!("group={gid}")),
                );
                settings.log_event(
                    now_epoch(),
                    EventKind::AdminAdd,
                    None,
                    Some(uid),
                    Some(&format!("group={gid} via=invite")),
                );
                settings.set_current_group(uid, gid);
                // multi по факту: пользователь мог уже быть админом других
                // групп — кнопку смены группы прячем только при единственной.
                let multi = settings.admin_group_ids(uid).len() > 1;
                bot.send_message(msg.chat.id, i18n::joined_group(lang, &gname))
                    .parse_mode(ParseMode::Html)
                    .reply_markup(menu::ga_main_menu(lang, multi))
                    .await?;
                // Уведомить владельцев о новом админе.
                for owner in &cfg.admin_ids {
                    let _ = bot
                        .send_message(
                            ChatId(*owner),
                            i18n::owner_notified_join(settings.lang(*owner), uid, &gname),
                        )
                        .parse_mode(ParseMode::Html)
                        .await;
                    let _ = bot
                        .forward_message(ChatId(*owner), msg.chat.id, msg.id)
                        .await;
                }
            }
            crate::store::InviteUse::AlreadyAdmin(gid) => {
                let gname = settings.group(gid).map(|g| g.name).unwrap_or_default();
                let multi = settings.admin_group_ids(uid).len() > 1;
                bot.send_message(msg.chat.id, i18n::joined_group(lang, &gname))
                    .parse_mode(ParseMode::Html)
                    .reply_markup(menu::ga_main_menu(lang, multi))
                    .await?;
            }
            crate::store::InviteUse::Invalid => {
                bot.send_message(msg.chat.id, i18n::invite_invalid(lang))
                    .await?;
            }
        }
        dialogue.update(State::Idle).await?;
        return Ok(());
    }

    let role = resolve_role(uid, &cfg.admin_ids, &settings);
    let state = dialogue.get().await?.unwrap_or_default();
    if msg.text().is_some_and(|text| text == "/paysupport") {
        bot.send_message(msg.chat.id, "💳 Поддержка по платежам\n\nОпишите проблему с оплатой и, если есть, укажите номер заказа. Не отправляйте данные банковской карты.")
            .reply_markup(menu::support_category_menu()).await?;
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if (role.is_owner() || matches!(&role,Role::Staff(v) if v=="technical"))
        && msg.text().is_some_and(|v| v.starts_with("/find "))
    {
        let query = msg
            .text()
            .unwrap_or_default()
            .trim_start_matches("/find ")
            .trim();
        let rows = settings.search_clients(query, 30);
        let text = if rows.is_empty() {
            "Ничего не найдено.".into()
        } else {
            rows.into_iter()
                .map(|(name, owner, label)| {
                    format!(
                        "• {name} · {} · {}",
                        label.unwrap_or_else(|| "без устройства".into()),
                        owner
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "без владельца".into())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        bot.send_message(msg.chat.id, format!("🔎 Поиск «{query}»\n\n{text}"))
            .await?;
        return Ok(());
    }
    if role.is_owner() {
        if let Some(raw) = msg.text().and_then(|v| v.strip_prefix("/bulk_")) {
            let parts: Vec<_> = raw.split_whitespace().collect();
            if let (Some(command), Some(prefix)) = (parts.first(), parts.get(1)) {
                let names = vpn
                    .list()
                    .await
                    .unwrap_or_default()
                    .into_iter()
                    .map(|c| c.name)
                    .filter(|n| n.starts_with(prefix))
                    .take(100)
                    .collect::<Vec<_>>();
                let mut ok = 0usize;
                for name in &names {
                    let result = match *command {
                        "disable" => vpn.disable_client(name).await,
                        "enable" => vpn.enable_client(name).await,
                        "extend" => {
                            if let Some(seconds) = parts.get(2).and_then(|v| duration_seconds(v)) {
                                vpn.extend_client(name, seconds, now_epoch())
                                    .await
                                    .map(|_| ())
                            } else {
                                Err(crate::error::Error::Parse(
                                    "нужен срок, например 30d".into(),
                                ))
                            }
                        }
                        _ => Err(crate::error::Error::Parse(
                            "команда не поддерживается".into(),
                        )),
                    };
                    if result.is_ok() {
                        ok += 1;
                        settings.log_event(
                            now_epoch(),
                            EventKind::Modify,
                            Some(name),
                            Some(uid),
                            Some(&format!("bulk_{command}")),
                        );
                    }
                }
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "✅ Массовая операция завершена: {ok}/{} ключей.",
                        names.len()
                    ),
                )
                .await?;
                return Ok(());
            }
        }
    }
    if let State::AwaitingSupportMessage { category } = state.clone() {
        let subject = msg.text().unwrap_or("Вложение");
        let ticket = settings
            .open_support_ticket_in_category(uid, &category, subject, now_epoch())
            .unwrap_or(0);
        settings.add_support_message(
            ticket,
            uid,
            false,
            (msg.chat.id.0, msg.id.0),
            msg.text(),
            now_epoch(),
        );
        for owner in &cfg.admin_ids {
            let _ = bot.send_message(ChatId(*owner), format!("🆘 Обращение #{ticket} от пользователя {uid}\nОтветьте на пересланное сообщение командой /reply_{uid} и затем отправьте ответ.")).await;
            let _ = bot
                .forward_message(ChatId(*owner), msg.chat.id, msg.id)
                .await;
        }
        bot.send_message(
            msg.chat.id,
            format!("✅ Обращение #{ticket} передано. Администратор ответит в этом чате."),
        )
        .reply_markup(menu::customer_keyboard())
        .await?;
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if let State::AwaitingSupportReply { ticket_id, user_id } = state.clone() {
        bot.copy_message(ChatId(user_id), msg.chat.id, msg.id)
            .await?;
        settings.add_support_message(
            ticket_id,
            uid,
            true,
            (msg.chat.id.0, msg.id.0),
            msg.text(),
            now_epoch(),
        );
        bot.send_message(msg.chat.id, "✅ Ответ отправлен пользователю.")
            .reply_markup(menu::admin_keyboard())
            .await?;
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if let State::AwaitingBroadcast { audience } = state.clone() {
        bot.send_message(
            msg.chat.id,
            format!("Предпросмотр готов. Сегмент: {audience}. Для запуска напишите: ОТПРАВИТЬ"),
        )
        .reply_markup(menu::admin_keyboard())
        .await?;
        dialogue
            .update(State::AwaitingBroadcastConfirm {
                source_chat_id: msg.chat.id.0,
                source_message_id: msg.id.0,
                audience,
            })
            .await?;
        return Ok(());
    }
    if let State::AwaitingBroadcastConfirm {
        source_chat_id,
        source_message_id,
        audience,
    } = state.clone()
    {
        if msg.text().is_some_and(|v| v.trim() == "ОТПРАВИТЬ") {
            let mut delivered = 0;
            let mut failed = 0;
            let now = now_epoch();
            let recipients = settings
                .all_user_ids()
                .into_iter()
                .filter(|user_id| {
                    let keys = settings.user_client_names(*user_id);
                    match audience.as_str() {
                        "active" => keys.iter().any(|n| {
                            !vpn.client_disabled(n) && vpn.client_expiry(n).is_none_or(|e| e > now)
                        }),
                        "expiring" => keys.iter().any(|n| {
                            vpn.client_expiry(n)
                                .is_some_and(|e| e > now && e - now <= 7 * 86_400)
                        }),
                        "nokeys" => keys.is_empty(),
                        value if value.starts_with("server:") => value
                            .strip_prefix("server:")
                            .and_then(|value| value.parse::<i64>().ok())
                            .is_some_and(|server_id| {
                                keys.iter().any(|name| {
                                    settings
                                        .client_vpn_server(name)
                                        .is_some_and(|server| server.id == server_id)
                                })
                            }),
                        _ => true,
                    }
                })
                .collect::<Vec<_>>();
            let broadcast_id = settings.create_broadcast_run(
                uid,
                source_chat_id,
                source_message_id,
                &audience,
                &recipients,
                now,
            );
            for user_id in recipients {
                match bot
                    .copy_message(
                        ChatId(user_id),
                        ChatId(source_chat_id),
                        MessageId(source_message_id),
                    )
                    .await
                {
                    Ok(_) => {
                        delivered += 1;
                        if let Some(id) = broadcast_id {
                            settings.record_broadcast_delivery(
                                id,
                                user_id,
                                true,
                                None,
                                now_epoch(),
                            );
                        }
                    }
                    Err(error) => {
                        failed += 1;
                        if let Some(id) = broadcast_id {
                            settings.record_broadcast_delivery(
                                id,
                                user_id,
                                false,
                                Some(&error.to_string()),
                                now_epoch(),
                            );
                        }
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            }
            let report = bot.send_message(
                msg.chat.id,
                format!(
                    "✅ Рассылка завершена: доставлено {delivered}, ошибок {failed}.{}",
                    broadcast_id
                        .map(|id| format!("\nНомер отчёта: #{id}"))
                        .unwrap_or_default()
                ),
            );
            if let Some(id) = broadcast_id {
                report
                    .reply_markup(menu::broadcast_report_menu(id, failed > 0))
                    .await?;
            } else {
                report.reply_markup(menu::admin_dashboard_menu()).await?;
            }
            settings.log_event(
                now_epoch(),
                EventKind::Broadcast,
                None,
                Some(uid),
                Some(&format!(
                    "audience={audience} delivered={delivered} failed={failed}"
                )),
            );
        } else {
            bot.send_message(msg.chat.id, "Рассылка отменена.")
                .reply_markup(menu::admin_keyboard())
                .await?;
        }
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if role.is_owner() {
        if let Some(args) = msg.text().and_then(|v| v.strip_prefix("/role ")) {
            let parts: Vec<_> = args.split_whitespace().collect();
            if let (Some(user_id), Some(value)) = (
                parts.first().and_then(|v| v.parse::<i64>().ok()),
                parts.get(1),
            ) {
                let selected = if *value == "remove" {
                    None
                } else {
                    Some(*value)
                };
                if settings.set_staff_role(user_id, selected, uid, now_epoch()) {
                    settings.log_event(
                        now_epoch(),
                        EventKind::RoleChange,
                        None,
                        Some(uid),
                        Some(&format!("user={user_id} role={value}")),
                    );
                    bot.send_message(
                        msg.chat.id,
                        format!("✅ Роль пользователя {user_id}: {value}"),
                    )
                    .reply_markup(menu::admin_keyboard())
                    .await?;
                } else {
                    bot.send_message(
                        msg.chat.id,
                        "Использование: /role TELEGRAM_ID technical|support|finance|remove",
                    )
                    .await?;
                }
                return Ok(());
            }
        }
        if let Some(target) = msg
            .text()
            .and_then(|v| v.strip_prefix("/reply_"))
            .and_then(|v| v.parse::<i64>().ok())
        {
            bot.send_message(
                msg.chat.id,
                format!("Отправьте ответ пользователю {target}:"),
            )
            .await?;
            let ticket_id = settings
                .support_tickets("open", 100)
                .into_iter()
                .chain(settings.support_tickets("in_progress", 100))
                .find(|t| t.user_id == target)
                .map(|t| t.id)
                .unwrap_or(0);
            dialogue
                .update(State::AwaitingSupportReply {
                    ticket_id,
                    user_id: target,
                })
                .await?;
            return Ok(());
        }
        match msg.text().unwrap_or_default() {
            "/start" | "🏠 Админ-панель" => {
                admin_dashboard(&bot, msg.chat.id, &vpn, &settings).await?;
                return Ok(());
            }
            "🔎 Поиск" => {
                bot.send_message(
                    msg.chat.id,
                    "Введите имя ключа, устройство, Telegram ID или username владельца:",
                )
                .await?;
                dialogue.update(State::AwaitingAdminSearch).await?;
                return Ok(());
            }
            "🚨 События" => {
                let servers = settings.vpn_servers();
                let alerts = servers
                    .iter()
                    .filter(|s| {
                        matches!(s.status.as_str(), "warning" | "offline")
                            || s.paid_until.is_some_and(|v| v <= now_epoch() + 7 * 86_400)
                    })
                    .count();
                bot.send_message(msg.chat.id,format!("🚨 События\n\nСерверы требуют внимания: {alerts}\nОжидают оплаты: {}\nОткрытые обращения: {}",settings.pending_payments().len(),settings.open_support_count())).reply_markup(menu::admin_dashboard_menu()).await?;
                return Ok(());
            }
            "👤 Кабинет" => {
                bot.send_message(msg.chat.id,format!("👤 Администратор\nTelegram ID: {uid}\nЛичный баланс: {:.2} ₽\nЛичных ключей: {}",settings.balance_kopecks(uid) as f64/100.0,settings.user_client_names(uid).len())).reply_markup(menu::admin_keyboard()).await?;
                return Ok(());
            }
            "👥 Клиенты" => {
                bot.send_message(msg.chat.id, i18n::menu_title(settings.lang(uid)))
                    .reply_markup(menu::main_menu(settings.lang(uid)))
                    .await?;
                return Ok(());
            }
            "💳 Финансы" => {
                finance_screen(&bot, msg.chat.id, &settings).await?;
                return Ok(());
            }
            "🔗 Владельцы" => {
                owners_screen(&bot, msg.chat.id, &settings, 0).await?;
                return Ok(());
            }
            "📣 Рассылка" => {
                bot.send_message(msg.chat.id, "Выберите получателей рассылки:")
                    .reply_markup(menu::broadcast_audience_menu())
                    .await?;
                return Ok(());
            }
            "🆘 Обращения" => {
                support_screen(&bot, msg.chat.id, &settings).await?;
                return Ok(());
            }
            "⚙️ Настройки" => {
                bot.send_message(
                    msg.chat.id,
                    i18n::settings_title(
                        settings.lang(uid),
                        settings.psk_default(),
                        settings.name_slug(),
                        settings.deliver_conf(),
                        settings.deliver_qr(),
                        settings.deliver_link(),
                    ),
                )
                .reply_markup(menu::settings_menu(
                    settings.lang(uid),
                    settings.psk_default(),
                    settings.name_slug(),
                    settings.deliver_conf(),
                    settings.deliver_qr(),
                    settings.deliver_link(),
                ))
                .await?;
                return Ok(());
            }
            _ => {}
        }
    }
    if let Role::Staff(staff_role) = &role {
        if matches!(msg.text(), Some("/start") | Some("🏠 Админ-панель")) {
            match staff_role.as_str() {
                "support" => {
                    let mut tickets = settings.support_tickets("open", 25);
                    tickets.extend(settings.support_tickets("in_progress", 25));
                    bot.send_message(msg.chat.id, "🆘 Рабочее место поддержки")
                        .reply_markup(menu::support_tickets_menu(&tickets))
                        .await?;
                }
                "finance" => {
                    bot.send_message(msg.chat.id, "💳 Финансовый сотрудник")
                        .reply_markup(menu::finance_menu())
                        .await?;
                }
                "technical" => {
                    bot.send_message(msg.chat.id, i18n::menu_title(settings.lang(uid)))
                        .reply_markup(menu::main_menu(settings.lang(uid)))
                        .await?;
                }
                _ => {}
            }
            return Ok(());
        }
    }
    if let State::AwaitingPaymentReject { id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let reason = msg.text().unwrap_or_default().trim();
        if settings.reject_payment(id, uid, reason, now_epoch()) {
            if let Some(req) = settings.payment_request(id) {
                let _ = bot
                    .send_message(
                        ChatId(req.user_id),
                        format!("❌ Заявка #{} отклонена.\nПричина: {reason}", req.id),
                    )
                    .reply_markup(menu::customer_keyboard())
                    .await;
            }
            bot.send_message(msg.chat.id, "✅ Заявка отклонена, пользователь уведомлён.")
                .reply_markup(menu::finance_menu())
                .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Укажите причину длиной до 500 символов либо заявка уже обработана.",
            )
            .await?;
        }
        return Ok(());
    }
    if let State::AwaitingPaymentProof { id } = state.clone() {
        let proof = msg
            .text()
            .or_else(|| msg.caption())
            .unwrap_or("Чек приложен сообщением");
        if settings.set_payment_proof(id, uid, proof) {
            if let Some(req) = settings.payment_request(id) {
                let user = settings.user(uid);
                let label = user
                    .as_ref()
                    .and_then(|u| u.username.as_ref().map(|v| format!("@{v}")))
                    .unwrap_or_else(|| uid.to_string());
                let text = format!(
                    "💳 Новая заявка #{}\nПользователь: {}\nTelegram ID: {}\nТариф: {} мес.\nСумма: {} ₽\nПодтверждение: {}",
                    req.id,
                    label,
                    uid,
                    req.months,
                    req.amount_kopecks / 100,
                    crate::i18n::html_escape(proof)
                );
                for owner in &cfg.admin_ids {
                    let _ = bot
                        .send_message(ChatId(*owner), &text)
                        .reply_markup(menu::payment_admin_menu(id))
                        .parse_mode(ParseMode::Html)
                        .await;
                    // Текст, фото или документ с чеком должны дойти до
                    // администратора в исходном виде, а не превращаться в
                    // безликую строку «подтверждение отправлено».
                    let _ = bot
                        .forward_message(ChatId(*owner), msg.chat.id, msg.id)
                        .await;
                }
            }
            bot.send_message(msg.chat.id, format!("✅ Чек по заявке #{id} отправлен администратору.\n\nПосле проверки бот автоматически пришлёт рабочий ключ."))
                .reply_markup(menu::customer_keyboard())
                .await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Заявка уже обработана либо принадлежит другому пользователю.",
            )
            .reply_markup(menu::customer_keyboard())
            .await?;
        }
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if matches!(&state, State::AwaitingTopupAmount) {
        let rubles = msg.text().unwrap_or_default().trim().parse::<i64>().ok();
        if let Some(rubles) = rubles.filter(|v| (100..=100_000).contains(v)) {
            if let Some(id) =
                settings.create_payment_request(uid, 0, rubles * 100, "topup", now_epoch())
            {
                bot.send_message(
                    msg.chat.id,
                    format!(
                        "Заявка на пополнение #{id}\nСумма: {rubles} ₽\n\n{}",
                        settings.payment_instructions()
                    ),
                )
                .reply_markup(menu::payment_paid_menu(id))
                .await?;
                dialogue.update(State::Idle).await?;
            }
        } else {
            bot.send_message(msg.chat.id, "Введите сумму от 100 до 100000 рублей.")
                .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingPaymentInstructions) {
        let value = msg.text().unwrap_or_default().trim();
        if !value.is_empty() && value.len() <= 1000 {
            settings.set_payment_instructions(value);
            bot.send_message(msg.chat.id, "✅ Текст реквизитов обновлён.")
                .reply_markup(menu::main_menu(settings.lang(uid)))
                .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(msg.chat.id, "Введите текст длиной от 1 до 1000 символов.")
                .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingAcquiringUrl) {
        let value = msg.text().unwrap_or_default().trim();
        if value.eq_ignore_ascii_case("off") {
            settings.set_acquiring_url_template(None);
            bot.send_message(msg.chat.id, "✅ Онлайн-эквайринг отключён.")
                .reply_markup(menu::admin_commerce_menu())
                .await?;
            dialogue.update(State::Idle).await?;
        } else if reqwest::Url::parse(value).is_ok() && value.contains("{order_id}") {
            settings.set_acquiring_url_template(Some(value));
            bot.send_message(msg.chat.id, "✅ Шаблон платёжной ссылки сохранён.")
                .reply_markup(menu::admin_commerce_menu())
                .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Нужен полный HTTPS/HTTP URL с {order_id}. Для отключения отправьте off.",
            )
            .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingTariffPricesRub) {
        let values = msg
            .text()
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|value| value.parse::<i64>().ok())
            .collect::<Vec<_>>();
        if values.len() == 4 && values.iter().all(|value| (1..=1_000_000).contains(value)) {
            settings.set_tariff_prices_kopecks([
                values[0] * 100,
                values[1] * 100,
                values[2] * 100,
                values[3] * 100,
            ]);
            bot.send_message(msg.chat.id, "✅ Рублёвые тарифы обновлены.")
                .reply_markup(menu::admin_commerce_menu())
                .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Введите четыре целых цены через пробел: 200 600 1000 2000",
            )
            .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingTariffPricesStars) {
        let values = msg
            .text()
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|value| value.parse::<i64>().ok())
            .collect::<Vec<_>>();
        if values.len() == 4 && values.iter().all(|value| (1..=1_000_000).contains(value)) {
            settings.set_tariff_prices_stars([values[0], values[1], values[2], values[3]]);
            bot.send_message(msg.chat.id, "✅ Тарифы Telegram Stars обновлены.")
                .reply_markup(menu::admin_commerce_menu())
                .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Введите четыре целых количества Stars через пробел, например: 100 250 450 800",
            )
            .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingReferralPercent) {
        if let Some(value) = msg
            .text()
            .unwrap_or_default()
            .trim()
            .parse::<u8>()
            .ok()
            .filter(|v| *v <= 100)
        {
            settings.set_referral_percent(value);
            bot.send_message(
                msg.chat.id,
                format!("✅ Реферальное вознаграждение: {value}%."),
            )
            .reply_markup(menu::admin_commerce_menu())
            .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(msg.chat.id, "Введите целое число от 0 до 100.")
                .await?;
        }
        return Ok(());
    }
    if let State::AwaitingAdminExpiry { name } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default().trim().to_ascii_lowercase();
        let result = if matches!(raw.as_str(), "none" | "без срока" | "бессрочно")
        {
            match settings.client_vpn_server(&name) {
                Some(server) if !server.is_local => Err(crate::error::Error::Parse(
                    "панель не поддерживает снятие срока через этот API; укажите новый срок".into(),
                )),
                _ => vpn.set_client_expiry(&name, None).await.map(|_| None),
            }
        } else {
            match duration_seconds(&raw).filter(|v| *v <= 10 * 31_536_000) {
                Some(seconds) => {
                    extend_managed_client(&vpn, &settings, &name, seconds, now_epoch())
                        .await
                        .map(Some)
                }
                None => Err(crate::error::Error::Parse(
                    "используйте 12h, 7d, 30d, 6m, 1y или none".into(),
                )),
            }
        };
        match result {
            Ok(Some(epoch)) => {
                bot.send_message(
                    msg.chat.id,
                    format!("✅ Срок ключа {name} изменён. Новая дата (Unix): {epoch}"),
                )
                .reply_markup(menu::admin_keyboard())
                .await?;
            }
            Ok(None) => {
                bot.send_message(msg.chat.id, format!("✅ Ключ {name} теперь бессрочный."))
                    .reply_markup(menu::admin_keyboard())
                    .await?;
            }
            Err(error) => {
                bot.send_message(msg.chat.id, i18n::error_text(settings.lang(uid), &error))
                    .await?;
            }
        }
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if let State::AwaitingDeviceLabel { name } = state.clone() {
        if settings.set_device_label(&name, uid, msg.text().unwrap_or_default()) {
            bot.send_message(
                msg.chat.id,
                format!("✅ Название устройства для ключа {name} сохранено."),
            )
            .reply_markup(menu::customer_keyboard())
            .await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Название должно содержать от 1 до 40 символов, а ключ должен принадлежать вам.",
            )
            .await?;
        }
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if matches!(&state, State::AwaitingAdminSearch) {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let query = msg.text().unwrap_or_default().trim();
        let rows = settings.search_clients(query, 30);
        let text = if rows.is_empty() {
            "Ничего не найдено.".into()
        } else {
            rows.into_iter()
                .map(|(name, owner, label)| {
                    format!(
                        "• {name} · {} · {}",
                        label.unwrap_or_else(|| "без устройства".into()),
                        owner
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "без владельца".into())
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };
        bot.send_message(
            msg.chat.id,
            format!("🔎 Результаты поиска «{query}»\n\n{text}"),
        )
        .reply_markup(menu::admin_dashboard_menu())
        .await?;
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if let State::AwaitingStaffRole { operation } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let parts = msg
            .text()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let selected = parts
            .first()
            .and_then(|v| v.parse::<i64>().ok())
            .and_then(|user_id| {
                if operation == "remove" {
                    Some((user_id, "remove"))
                } else {
                    parts.get(1).map(|v| (user_id, v.as_str()))
                }
            });
        let result = selected.is_some_and(|(user_id, value)| {
            let changed = settings.set_staff_role(
                user_id,
                if value == "remove" { None } else { Some(value) },
                uid,
                now_epoch(),
            );
            if changed {
                settings.log_event(
                    now_epoch(),
                    EventKind::RoleChange,
                    None,
                    Some(uid),
                    Some(&format!("user={user_id} role={value}")),
                );
            }
            changed
        });
        bot.send_message(
            msg.chat.id,
            if result {
                "✅ Роль обновлена."
            } else {
                "Не удалось изменить роль. Для добавления: TELEGRAM_ID technical|support|finance; для удаления достаточно Telegram ID."
            },
        )
        .reply_markup(menu::admin_dashboard_menu())
        .await?;
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if let State::AwaitingBulkManage { operation } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let parts = msg
            .text()
            .unwrap_or_default()
            .split_whitespace()
            .map(str::to_string)
            .collect::<Vec<_>>();
        let prefix = parts.first().cloned().unwrap_or_default();
        let seconds = parts.get(1).and_then(|v| duration_seconds(v));
        if prefix.is_empty() || (operation == "extend" && seconds.is_none()) {
            bot.send_message(
                msg.chat.id,
                "Введите префикс; для продления также срок. Например: client 30d",
            )
            .await?;
            return Ok(());
        }
        let names = vpn
            .list()
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|c| c.name)
            .filter(|n| n.starts_with(&prefix))
            .take(100)
            .collect::<Vec<_>>();
        let preview = names
            .iter()
            .take(10)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ");
        bot.send_message(
            msg.chat.id,
            format!(
                "⚠️ Предпросмотр массовой операции\n\nОперация: {operation}\nПрефикс: {prefix}\nБудет затронуто: {}\nПервые ключи: {}{}",
                names.len(),if preview.is_empty(){"—"}else{&preview},if names.len()>10{" …"}else{""}
            ),
        )
        .reply_markup(menu::bulk_confirm_menu())
        .await?;
        dialogue
            .update(State::AwaitingBulkConfirm {
                operation,
                prefix,
                names,
                seconds,
            })
            .await?;
        return Ok(());
    }
    if let State::AwaitingUserBalance { user_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default().trim();
        let mut parts = raw.splitn(2, char::is_whitespace);
        let value = parts
            .next()
            .unwrap_or_default()
            .replace(',', ".")
            .parse::<f64>()
            .ok();
        let reason = parts
            .next()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("Корректировка администратором");
        let kopecks = value
            .map(|v| (v * 100.0).round() as i64)
            .filter(|v| *v != 0);
        let changed = kopecks.is_some_and(|amount| {
            settings.add_ledger_entry(
                user_id,
                amount,
                "admin_adjustment",
                &format!("admin:{uid}:{user_id}:{}", now_epoch()),
                Some(reason),
                now_epoch(),
            )
        });
        bot.send_message(
            msg.chat.id,
            if changed {
                "✅ Баланс скорректирован."
            } else {
                "Введите ненулевую сумму в рублях, например 500 или -200."
            },
        )
        .reply_markup(menu::admin_user_menu(
            user_id,
            settings.user_blocked(user_id),
        ))
        .await?;
        if changed {
            let amount = kopecks.unwrap_or_default();
            let operation = if amount > 0 {
                "пополнен"
            } else {
                "уменьшен"
            };
            let _ = bot
                .send_message(
                    ChatId(user_id),
                    format!(
                        "💰 Ваш баланс {operation} на {:.2} ₽.\nПричина: {reason}\nТекущий баланс: {:.2} ₽.",
                        amount.unsigned_abs() as f64 / 100.0,
                        settings.balance_kopecks(user_id) as f64 / 100.0
                    ),
                )
                .reply_markup(menu::customer_keyboard())
                .await;
            dialogue.update(State::Idle).await?;
        }
        return Ok(());
    }
    if let State::AwaitingUserDiscount { user_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default().trim();
        let parts = raw.split_whitespace().collect::<Vec<_>>();
        let value = if raw.eq_ignore_ascii_case("clear") {
            Some((None, None))
        } else {
            parts
                .first()
                .and_then(|part| part.parse::<i64>().ok())
                .filter(|v| (0..=100).contains(v))
                .map(|discount| {
                    let until = parts
                        .get(1)
                        .and_then(|date| crate::calendar::parse_date(date));
                    (Some(discount), until)
                })
        };
        let changed = value.is_some_and(|(discount, until)| {
            settings.set_personal_discount(user_id, discount, until)
        });
        bot.send_message(
            msg.chat.id,
            if changed {
                "✅ Индивидуальная скидка сохранена."
            } else {
                "Формат: 25 (бессрочно), 25 2027-12-31 или clear."
            },
        )
        .reply_markup(menu::admin_user_menu(
            user_id,
            settings.user_blocked(user_id),
        ))
        .await?;
        if changed {
            dialogue.update(State::Idle).await?;
        }
        return Ok(());
    }
    if let State::AwaitingUserNote { user_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default().trim();
        let note = if raw.eq_ignore_ascii_case("clear") {
            None
        } else {
            Some(raw)
        };
        let changed = raw.chars().count() <= 500 && settings.set_user_note(user_id, note);
        bot.send_message(
            msg.chat.id,
            if changed {
                "✅ Заметка сохранена."
            } else {
                "Заметка не сохранена: максимум 500 символов."
            },
        )
        .reply_markup(menu::admin_user_menu(
            user_id,
            settings.user_blocked(user_id),
        ))
        .await?;
        if changed {
            dialogue.update(State::Idle).await?;
        }
        return Ok(());
    }
    if let State::AwaitingClientNote { name } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default().trim();
        let note = if raw.eq_ignore_ascii_case("clear") {
            None
        } else {
            Some(raw)
        };
        let changed = raw.chars().count() <= 500 && settings.set_client_note(&name, note);
        bot.send_message(
            msg.chat.id,
            if changed {
                "✅ Заметка ключа сохранена."
            } else {
                "Не удалось сохранить заметку."
            },
        )
        .reply_markup(menu::admin_dashboard_menu())
        .await?;
        if changed {
            dialogue.update(State::Idle).await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingCustomerPromo) {
        let text = msg.text().unwrap_or_default().trim();
        let navigation = is_customer_navigation(text);
        dialogue.update(State::Idle).await?;
        if !navigation {
            if settings.activate_legacy_promo(uid, text, now_epoch())
                || settings.has_pending_legacy_entitlement(uid, text, now_epoch())
            {
                bot.send_message(msg.chat.id,"✅ Право на восстановление подтверждено. Откройте раздел «♻️ Восстановить ключи» внизу. Там можно отправлять заявки на необходимое количество ранее купленных у администратора ключей.").reply_markup(menu::customer_keyboard()).await?;
                return Ok(());
            }
            match settings.activate_promo(uid, text, now_epoch()) {
                Some(discount) => {
                    bot.send_message(msg.chat.id,format!("✅ Промокод активирован. Скидка {discount}% применится к следующей покупке или продлению.")).reply_markup(menu::customer_keyboard()).await?;
                }
                None => {
                    bot.send_message(
                        msg.chat.id,
                        "Промокод не найден, истёк, исчерпан или уже использован вами. Попробуйте снова через кнопку «🎟 Промокод».",
                    )
                    .reply_markup(menu::customer_keyboard())
                    .await?;
                }
            }
            return Ok(());
        }
    }
    if matches!(&state, State::AwaitingLegacyRequest) {
        let text = msg.text().unwrap_or_default().trim();
        if is_customer_navigation(text) {
            dialogue.update(State::Idle).await?;
        } else {
            let now = now_epoch();
            let mut parts = text.splitn(2, char::is_whitespace);
            let requested = parts.next().unwrap_or_default();
            let comment = parts.next().map(str::trim).filter(|v| !v.is_empty());
            if let Some(id) = settings.create_legacy_request(uid, requested, comment, now) {
                let user = settings.user(uid);
                let username = user
                    .as_ref()
                    .and_then(|u| u.username.as_deref())
                    .map(|v| format!("@{v}"))
                    .unwrap_or_else(|| "без username".into());
                for owner in &cfg.admin_ids {
                    let _=bot.send_message(ChatId(*owner),format!("♻️ Новая заявка на восстановление #{id}\nПользователь: {username}\nTelegram ID: {uid}\nЖелаемое имя: {requested}\nКомментарий: {}",comment.unwrap_or("—"))).reply_markup(menu::legacy_request_admin_menu(id)).await;
                }
                bot.send_message(msg.chat.id,format!("✅ Заявка #{id} отправлена на ручную проверку. После подтверждения новый ключ появится в этом чате. Вы можете отправить ещё одну заявку через раздел восстановления.")).reply_markup(menu::customer_keyboard()).await?;
                dialogue.update(State::Idle).await?;
            } else {
                bot.send_message(msg.chat.id,"Не удалось создать заявку. Проверьте название, активируйте технический промокод или убедитесь, что срок подачи ещё не завершён.").reply_markup(menu::customer_keyboard()).await?;
            }
            return Ok(());
        }
    }
    if let State::AwaitingPromoCode { kind } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let p = msg
            .text()
            .unwrap_or_default()
            .split_whitespace()
            .collect::<Vec<_>>();
        let changed = if kind == "legacy" {
            settings.create_legacy_promo(
                p.first().copied().unwrap_or_default(),
                p.get(1).and_then(|v| v.parse().ok()),
                uid,
                now_epoch(),
            )
        } else {
            settings.create_promo(
                p.first().copied().unwrap_or_default(),
                p.get(1).and_then(|v| v.parse().ok()).unwrap_or(0),
                p.get(2).and_then(|v| v.parse().ok()),
                None,
                uid,
                now_epoch(),
            )
        };
        bot.send_message(
            msg.chat.id,
            if changed {
                "✅ Промокод создан."
            } else {
                "Неверный формат. Скидочный: CODE PERCENT [MAX_USES]. Legacy: CODE [MAX_USES]."
            },
        )
        .reply_markup(menu::admin_dashboard_menu())
        .await?;
        if changed {
            dialogue.update(State::Idle).await?;
        }
        return Ok(());
    }
    if let State::AwaitingLegacyReject { id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let reason = msg.text().unwrap_or_default().trim();
        let request = settings.legacy_request(id);
        if !reason.is_empty()
            && reason.chars().count() <= 500
            && settings.decide_legacy_request(id, uid, None, Some(reason), now_epoch())
        {
            if let Some(request) = request {
                let _ = bot
                    .send_message(
                        ChatId(request.user_id),
                        format!("❌ Заявка на восстановление #{id} отклонена.\nПричина: {reason}"),
                    )
                    .reply_markup(menu::customer_keyboard())
                    .await;
            }
            bot.send_message(msg.chat.id, "✅ Заявка отклонена.")
                .reply_markup(menu::admin_dashboard_menu())
                .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Введите причину отказа длиной до 500 символов.",
            )
            .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingLegacyPrice) {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let value = msg
            .text()
            .unwrap_or_default()
            .trim()
            .replace(',', ".")
            .parse::<f64>()
            .ok()
            .map(|v| (v * 100.0).round() as i64)
            .filter(|v| *v >= 0);
        if let Some(value) = value {
            settings.set_legacy_renewal_price_kopecks(value);
            bot.send_message(
                msg.chat.id,
                format!(
                    "✅ Цена технического продления: {:.2} ₽",
                    value as f64 / 100.0
                ),
            )
            .reply_markup(menu::admin_dashboard_menu())
            .await?;
            dialogue.update(State::Idle).await?;
        } else {
            bot.send_message(msg.chat.id, "Введите цену в рублях, например 1000.")
                .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingServerWizardName) {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let name = msg.text().unwrap_or_default().trim();
        if name.is_empty() || name.chars().count() > 64 {
            bot.send_message(
                msg.chat.id,
                "Название должно содержать от 1 до 64 символов.",
            )
            .await?;
        } else {
            dialogue
                .update(State::AwaitingServerWizardAddress {
                    name: name.to_owned(),
                })
                .await?;
            bot.send_message(
                msg.chat.id,
                "Шаг 2 из 3. Отправьте публичный IP-адрес нового VPS.",
            )
            .await?;
        }
        return Ok(());
    }
    if let State::AwaitingServerWizardAddress { name } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let public_ip = msg.text().unwrap_or_default().trim();
        if public_ip.parse::<std::net::IpAddr>().is_err() {
            bot.send_message(
                msg.chat.id,
                "Нужен корректный IPv4 или IPv6 без http:// и номера порта.",
            )
            .await?;
        } else {
            dialogue
                .update(State::AwaitingServerWizardDetails {
                    name,
                    public_ip: public_ip.to_owned(),
                })
                .await?;
            bot.send_message(
                msg.chat.id,
                "Шаг 3 из 3. Отправьте:\nЛОКАЦИЯ | ХОСТЕР\n\nПример: Amsterdam | Hoster",
            )
            .await?;
        }
        return Ok(());
    }
    if let State::AwaitingServerWizardDetails { name, public_ip } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let parts = msg
            .text()
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        if parts.len() != 2 || parts.iter().any(|part| part.is_empty()) {
            bot.send_message(msg.chat.id, "Неверный формат. Отправьте: ЛОКАЦИЯ | ХОСТЕР")
                .await?;
            return Ok(());
        }
        let created = settings.add_vpn_server(
            &crate::store::NewVpnServer {
                name: &name,
                hostname: &public_ip,
                public_ip: &public_ip,
                provider: parts[1],
                location: parts[0],
                protocol: "amneziawg-1",
                opened_at: Some(now_epoch()),
                is_local: false,
            },
            uid,
            now_epoch(),
        );
        dialogue.update(State::Idle).await?;
        if let Some(id) = created {
            bot.send_message(msg.chat.id, format!("✅ Сервер «{name}» добавлен.\n\nВыберите способ подключения. До успешной проверки сервер не участвует в выдаче новых ключей."))
                .reply_markup(menu::server_setup_method_menu(id))
                .await?;
        } else {
            bot.send_message(msg.chat.id, "❌ Не удалось создать сервер.")
                .reply_markup(menu::servers_menu(&settings.vpn_servers()))
                .await?;
        }
        return Ok(());
    }
    if matches!(&state, State::AwaitingServerAdd) {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let parts = msg
            .text()
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        let created = if parts.len() == 7 {
            let value = crate::store::NewVpnServer {
                name: parts[0],
                hostname: parts[1],
                public_ip: parts[2],
                provider: parts[3],
                location: parts[4],
                protocol: parts[5],
                opened_at: crate::calendar::parse_date(parts[6]),
                is_local: false,
            };
            settings.add_vpn_server(&value, uid, now_epoch())
        } else {
            None
        };
        if let Some(id) = created {
            dialogue.update(State::Idle).await?;
            let server = settings.vpn_server(id).expect("server was just inserted");
            bot.send_message(msg.chat.id, server_card_text(&server, now_epoch()))
                .reply_markup(menu::server_card_menu(id))
                .await?;
        } else {
            bot.send_message(msg.chat.id,"Не удалось сохранить. Допустимы только: amneziawg-2, amneziawg-1 или amneziawg-panel.").await?;
        }
        return Ok(());
    }
    if let State::AwaitingServerBilling { server_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let parts = msg
            .text()
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        let changed = if parts.len() == 5 {
            match (
                crate::calendar::parse_date(parts[0]),
                parts[1].parse::<i64>().ok(),
                parse_minor(parts[2]),
            ) {
                (Some(paid_until), Some(period_months), Some(cost_minor)) => settings
                    .update_server_billing(
                        server_id,
                        &crate::store::ServerBillingUpdate {
                            paid_until,
                            period_months,
                            cost_minor,
                            currency: parts[3],
                            auto_renew: matches!(
                                parts[4].to_ascii_lowercase().as_str(),
                                "да" | "yes" | "on" | "1"
                            ),
                        },
                        now_epoch(),
                    ),
                _ => false,
            }
        } else {
            false
        };
        if changed {
            dialogue.update(State::Idle).await?;
            let server = settings
                .vpn_server(server_id)
                .expect("updated server exists");
            bot.send_message(msg.chat.id, server_card_text(&server, now_epoch()))
                .reply_markup(menu::server_card_menu(server_id))
                .await?;
        } else {
            bot.send_message(
                msg.chat.id,
                "Неверный формат. Пример: 2026-09-15 | 1 | 6.00 | EUR | да",
            )
            .await?;
        }
        return Ok(());
    }
    if let State::AwaitingServerPassport { server_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let parts = msg
            .text()
            .unwrap_or_default()
            .split('|')
            .map(str::trim)
            .collect::<Vec<_>>();
        let changed = if parts.len() == 7 {
            settings.update_server_passport(
                server_id,
                &crate::store::NewVpnServer {
                    name: parts[0],
                    hostname: parts[1],
                    public_ip: parts[2],
                    provider: parts[3],
                    location: parts[4],
                    protocol: parts[5],
                    opened_at: crate::calendar::parse_date(parts[6]),
                    is_local: false,
                },
                now_epoch(),
            )
        } else {
            false
        };
        if changed {
            dialogue.update(State::Idle).await?;
            let server = settings
                .vpn_server(server_id)
                .expect("updated server exists");
            bot.send_message(msg.chat.id, server_card_text(&server, now_epoch()))
                .reply_markup(menu::server_card_menu(server_id))
                .await?;
        } else {
            bot.send_message(msg.chat.id,"Не удалось обновить паспорт. Допустимы только: amneziawg-2, amneziawg-1 или amneziawg-panel.").await?;
        }
        return Ok(());
    }
    if let State::AwaitingPanelCredentials { server_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default();
        let mut parts = raw.splitn(2, '|').map(str::trim);
        let url = parts.next().unwrap_or_default().to_owned();
        let mut password = parts.next().unwrap_or_default().to_owned();
        let _ = bot.delete_message(msg.chat.id, msg.id).await;
        let Some(server) = settings
            .vpn_server(server_id)
            .filter(|server| !server.is_local)
        else {
            password.clear();
            dialogue.update(State::Idle).await?;
            bot.send_message(msg.chat.id, "Удалённый сервер не найден.")
                .await?;
            return Ok(());
        };
        if url.is_empty() || password.is_empty() {
            password.clear();
            bot.send_message(msg.chat.id, "Неверный формат. Отправьте: URL | ПАРОЛЬ")
                .await?;
            return Ok(());
        }
        bot.send_message(msg.chat.id, "⏳ Проверяю панель и пароль…")
            .await?;
        let checked = vpn.test_panel(&url, &password).await;
        match checked {
            Ok(clients) => {
                let encrypted = match vpn.protect_panel_password(&password) {
                    Ok(value) => value,
                    Err(error) => {
                        password.clear();
                        dialogue.update(State::Idle).await?;
                        bot.send_message(
                            msg.chat.id,
                            format!("❌ Не удалось безопасно сохранить пароль: {error}"),
                        )
                        .await?;
                        return Ok(());
                    }
                };
                password.clear();
                if !settings.set_panel_credentials(server.id, &url, &encrypted, now_epoch()) {
                    bot.send_message(msg.chat.id, "❌ Не удалось сохранить подключение панели.")
                        .await?;
                } else {
                    let imported = settings.sync_panel_clients(
                        server.id,
                        &clients
                            .iter()
                            .map(|client| (client.name.clone(), client.address.clone()))
                            .collect::<Vec<_>>(),
                        now_epoch(),
                    );
                    settings.ingest_panel(
                        server.id,
                        now_epoch(),
                        &clients
                            .iter()
                            .map(|client| crate::store::Sample {
                                name: client.name.clone(),
                                ip: client.address.clone(),
                                rx: client.transfer_rx,
                                tx: client.transfer_tx,
                                last_handshake: client.last_handshake_epoch(),
                            })
                            .collect::<Vec<_>>(),
                    );
                    bot.send_message(msg.chat.id, format!("✅ Панель подключена. Найдено клиентов: {}; синхронизировано: {imported}.\n\n{}", clients.len(), if url.starts_with("http://") {"⚠️ Используется незашифрованный HTTP. Ограничьте порт панели по IP сервера бота."} else {"Соединение защищено HTTPS."}))
                        .reply_markup(menu::server_card_menu(server.id))
                        .await?;
                }
            }
            Err(error) => {
                password.clear();
                bot.send_message(msg.chat.id, format!("❌ Панель не подключена: {error}"))
                    .reply_markup(menu::server_card_menu(server.id))
                    .await?;
            }
        }
        dialogue.update(State::Idle).await?;
        return Ok(());
    }
    if let State::AwaitingServerDeployCredentials { server_id } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let raw = msg.text().unwrap_or_default();
        let mut parts = raw.splitn(2, '|').map(str::trim);
        let user = parts.next().unwrap_or_default();
        let mut password = parts.next().unwrap_or_default().to_owned();
        // Сообщение содержит секрет: удаляем его до сетевого запроса и никогда
        // не записываем пароль в БД или журнал.
        let _ = bot.delete_message(msg.chat.id, msg.id).await;
        let Some(server) = settings.vpn_server(server_id) else {
            password.clear();
            dialogue.update(State::Idle).await?;
            bot.send_message(msg.chat.id, "Сервер не найден.").await?;
            return Ok(());
        };
        if user != "root" || password.is_empty() {
            password.clear();
            bot.send_message(msg.chat.id, "Неверный формат. Отправьте: root | ПАРОЛЬ")
                .await?;
            return Ok(());
        }
        bot.send_message(
            msg.chat.id,
            format!(
                "⏳ Подключаюсь к {} и запускаю установку AWG 1.0…",
                server.public_ip
            ),
        )
        .await?;
        let Some(node) = settings.vpn_node_for_server(server.id) else {
            password.clear();
            dialogue.update(State::Idle).await?;
            bot.send_message(msg.chat.id, "VPN-узел сервера не найден.")
                .await?;
            return Ok(());
        };
        let (node_secret, encrypted_secret) = vpn.create_node_secret()?;
        let controller_key = vpn.controller_public_key_b64()?;
        let result = vpn
            .deploy_node(crate::vpn::DeployRequest {
                host: &server.public_ip,
                port: 22,
                user,
                password: &password,
                server_id: server.id,
                node_id: node.id,
                protocol: &server.protocol,
                node_secret_b64: &node_secret,
                controller_key_b64: &controller_key,
            })
            .await;
        password.clear();
        dialogue.update(State::Idle).await?;
        match result {
            Ok(_) => {
                settings.set_node_secret(server.id, &encrypted_secret, now_epoch());
                settings.set_server_status(server.id, "maintenance", now_epoch());
                settings.set_server_provisioning(server.id, false, now_epoch());
                bot.send_message(msg.chat.id, "✅ Установка запущена. Пароль удалён. VPS может перезагрузиться дважды; после готовности включите выдачу и назначьте сервер основным.")
                    .reply_markup(menu::server_card_menu(server.id)).await?;
            }
            Err(error) => {
                tracing::warn!(server_id = server.id, %error, "server deployment failed");
                bot.send_message(
                    msg.chat.id,
                    format!("❌ Не удалось запустить установку: {error}"),
                )
                .reply_markup(menu::server_card_menu(server.id))
                .await?;
            }
        }
        return Ok(());
    }
    if let State::AwaitingLocalMigrationConfirm { operation } = state.clone() {
        if !role.is_owner() {
            dialogue.update(State::Idle).await?;
            return Ok(());
        }
        let expected = if operation == "start" {
            "MIGRATE AWG1"
        } else {
            "ROLLBACK AWG2"
        };
        if msg.text().unwrap_or_default().trim() != expected {
            bot.send_message(
                msg.chat.id,
                format!("Подтверждение не совпало. Для продолжения отправьте точно: {expected}"),
            )
            .await?;
            return Ok(());
        }
        dialogue.update(State::Idle).await?;
        let command = if operation == "start" {
            "start"
        } else {
            "rollback"
        };
        bot.send_message(msg.chat.id, "⏳ Запускаю защищённую системную операцию…")
            .await?;
        match vpn.local_legacy_migration(command).await {
            Ok(output) => {
                if command == "start" {
                    settings.set_local_migration_notice_sent(false);
                }
                settings.log_event(
                    now_epoch(),
                    EventKind::Migration,
                    None,
                    Some(uid),
                    Some(&format!("local migration {command}")),
                );
                let text = if command == "start" {
                    "🚨 Миграция запущена. VPN будет временно недоступен, сервер дважды перезагрузится. Не запускайте повторную установку. После возвращения бота откройте «Статус миграции»."
                } else {
                    "✅ Команда отката выполнена. Проверьте состояние AWG 2.0."
                };
                bot.send_message(msg.chat.id, format!("{text}\n\nОтвет helper: {output}"))
                    .reply_markup(menu::local_migration_menu())
                    .await?;
            }
            Err(error) => {
                tracing::error!(%error, command, "local migration operation failed");
                bot.send_message(msg.chat.id, format!("❌ Операция не запущена: {error}"))
                    .reply_markup(menu::local_migration_menu())
                    .await?;
            }
        }
        return Ok(());
    }
    if role == Role::Denied && settings.user_blocked(uid) {
        if msg.text() == Some("🆘 Поддержка") {
            bot.send_message(msg.chat.id, "Выберите тему обращения:")
                .reply_markup(menu::support_category_menu())
                .await?;
        } else {
            bot.send_message(msg.chat.id,"⛔ Доступ к боту приостановлен. Обратитесь в поддержку, если считаете это ошибкой.").reply_markup(menu::customer_keyboard()).await?;
        }
        return Ok(());
    }
    if role == Role::Denied {
        match msg.text().unwrap_or_default() {
            text if text.starts_with("/start") || matches!(text, "🏠 Меню" | "🏠 Кабинет") =>
            {
                maybe_issue_trial(&bot, msg.chat.id, &vpn, &settings, uid).await;
                customer_dashboard(&bot, msg.chat.id, uid, &vpn, &settings).await?;
            }
            "➕ Купить ключ" => {
                let servers = settings.available_vpn_servers();
                bot.send_message(
                    msg.chat.id,
                    if servers.is_empty() {
                        "Сейчас нет доступных серверов для выдачи ключа."
                    } else {
                        "🌍 Шаг 1 из 3 · Выберите сервер подключения:"
                    },
                )
                .reply_markup(menu::buy_servers_menu(&servers, &settings))
                .await?;
            }
            "🔑 Мои ключи" => {
                let (lines, buttons) = customer_key_list(&settings, &vpn, uid);
                let text = if lines.is_empty() {
                    "🔑 У вас пока нет ключей. Вы можете приобрести ключ или обратиться в поддержку.".to_string()
                } else {
                    format!(
                        "🔑 Ваши подключения\n\n{}\n\n❌ Нерабочий ключ откройте и замените кнопкой восстановления.",
                        lines.join("\n\n")
                    )
                };
                let mut request = bot.send_message(msg.chat.id, text);
                if !buttons.is_empty() {
                    request = request.reply_markup(menu::customer_keys_menu(&buttons));
                }
                request.await?;
            }
            "➕ Пополнить" => {
                bot.send_message(
                    msg.chat.id,
                    "Введите сумму пополнения в рублях (от 100 до 100000):",
                )
                .await?;
                dialogue.update(State::AwaitingTopupAmount).await?;
            }
            "📖 Инструкция" => {
                bot.send_message(msg.chat.id, "📖 Инструкции по подключению\n\nВыберите приложение или откройте диагностику, если VPN уже настроен, но не подключается.").reply_markup(menu::instructions_menu()).await?;
            }
            "🆘 Поддержка" => {
                bot.send_message(msg.chat.id, "Выберите тему обращения:")
                    .reply_markup(menu::support_category_menu())
                    .await?;
            }
            "🎟 Промокод" => {
                bot.send_message(msg.chat.id, "Введите промокод:").await?;
                dialogue.update(State::AwaitingCustomerPromo).await?;
            }
            "♻️ Восстановить ключи" if crate::calendar::legacy_requests_open(now_epoch()) =>
            {
                let eligible = settings.legacy_user_eligible(uid, now_epoch());
                bot.send_message(msg.chat.id,"♻️ Восстановление ранее приобретённых ключей\n\nЕсли вы покупали ключи лично у администратора, здесь можно запросить создание такого же количества новых ключей. Восстановление бесплатно, но каждая заявка проверяется вручную. Администратор видит ваш @username и Telegram ID.\n\nЗаявки принимаются до 30.11.2026 включительно. Новый ключ действует до конца 2026 года; дальнейшее ежегодное продление оплачивается по отдельному техническому тарифу.").reply_markup(menu::legacy_restore_menu(eligible)).await?;
            }
            _ => {
                bot.send_message(msg.chat.id, "Используйте кнопки меню ниже.")
                    .reply_markup(menu::customer_keyboard())
                    .await?;
            }
        }
        return Ok(());
    }
    let lang = settings.lang(uid);
    match state {
        State::AwaitingName => {
            let name = msg.text().unwrap_or_default().to_string();
            match crate::vpn::validate::normalize_name(&name, None) {
                Ok(valid) => {
                    match vpn.exists(&valid).await {
                        Ok(false) => {
                            let confirm_line = match lang {
                                Lang::Ru => format!("Клиент: {valid}"),
                                Lang::En => format!("Client: {valid}"),
                            };
                            bot.send_message(
                                msg.chat.id,
                                format!("{confirm_line}\n{}", i18n::ask_expiry(lang)),
                            )
                            .reply_markup(menu::expiry_menu(lang))
                            .await?;
                            dialogue
                                .update(State::AwaitingExpiry {
                                    name: valid,
                                    recreate: false,
                                })
                                .await?;
                        }
                        Ok(true) => {
                            let suggestion = vpn.list().await.ok().and_then(|clients| {
                                let existing = clients
                                    .into_iter()
                                    .map(|c| c.name)
                                    .collect::<std::collections::HashSet<_>>();
                                crate::vpn::validate::gen_available_names(&valid, 1, &existing)
                                    .ok()
                                    .and_then(|mut names| names.pop())
                            });
                            if let Some(suggested) = suggestion {
                                bot.send_message(
                                    msg.chat.id,
                                    i18n::client_exists_suggest(lang, &valid, &suggested),
                                )
                                .reply_markup(menu::expiry_menu(lang))
                                .parse_mode(ParseMode::Html)
                                .await?;
                                dialogue
                                    .update(State::AwaitingExpiry {
                                        name: suggested,
                                        recreate: false,
                                    })
                                    .await?;
                                return Ok(());
                            }
                            // Клавиатуру «Пересоздать» показываем только если этот
                            // клиент в скоупе роли — иначе групповой админ увидел бы
                            // кнопку для чужого клиента (клик всё равно блокирует
                            // client_in_scope в Action::Recreate, но предлагать её
                            // нельзя).
                            let kb = if client_in_scope(&role, &settings, &valid) {
                                menu::confirm_recreate(lang, &valid)
                            } else {
                                home_menu(&role, lang)
                            };
                            bot.send_message(msg.chat.id, i18n::client_exists(lang, &valid))
                                .reply_markup(kb)
                                .parse_mode(ParseMode::Html)
                                .await?;
                            dialogue.update(State::Idle).await?;
                        }
                        Err(e) => {
                            // list --json упал — не блокируем создание (fail-open).
                            tracing::warn!(error = %e, "exists check failed, proceeding without duplicate guard");
                            bot.send_message(msg.chat.id, i18n::ask_expiry(lang))
                                .reply_markup(menu::expiry_menu(lang))
                                .await?;
                            dialogue
                                .update(State::AwaitingExpiry {
                                    name: valid,
                                    recreate: false,
                                })
                                .await?;
                        }
                    }
                }
                Err(_e) => {
                    bot.send_message(msg.chat.id, i18n::bad_name(lang, false))
                        .await?;
                }
            }
        }
        State::AwaitingCustomExpiry { name, recreate } => {
            let raw = msg.text().unwrap_or_default().to_string();
            match crate::vpn::validate::validate_expiry(&raw) {
                Ok(exp) => {
                    bot.send_message(msg.chat.id, i18n::psk_step(lang, settings.psk_default()))
                        .reply_markup(menu::psk_step(lang, settings.psk_default()))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    dialogue
                        .update(State::AwaitingPsk {
                            name,
                            expires: Some(exp),
                            recreate,
                        })
                        .await?;
                }
                Err(_e) => {
                    bot.send_message(msg.chat.id, i18n::bad_expiry(lang))
                        .await?;
                }
            }
        }
        State::AwaitingModifyValue { name, param } => {
            let raw = msg.text().unwrap_or_default().to_string();
            match crate::vpn::validate::parse_modify_value(param, &raw) {
                Ok(value) => {
                    let waiting = bot
                        .send_message(msg.chat.id, i18n::creating(lang))
                        .await
                        .ok();
                    match vpn.modify(&name, param, &value).await {
                        Ok(out) => {
                            settings.log_event(
                                now_epoch(),
                                EventKind::Modify,
                                Some(&name),
                                Some(uid),
                                Some(param.as_str()),
                            );
                            if let Some(m) = waiting {
                                let _ = bot.delete_message(msg.chat.id, m.id).await;
                            }
                            bot.send_message(
                                msg.chat.id,
                                i18n::modify_done(lang, param, &out.value),
                            )
                            .reply_markup(menu::main_menu(lang))
                            .parse_mode(ParseMode::Html)
                            .await?;
                        }
                        Err(e) => {
                            tracing::error!(error = %e, "modify провалился");
                            if let Some(m) = waiting {
                                let _ = bot.delete_message(msg.chat.id, m.id).await;
                            }
                            bot.send_message(msg.chat.id, i18n::error_text(lang, &e))
                                .await?;
                        }
                    }
                    dialogue.exit().await?;
                }
                Err(_e) => {
                    // Невалидный ввод — остаёмся в том же state, даём попробовать снова.
                    bot.send_message(
                        msg.chat.id,
                        format!("⚠️ {}", i18n::ask_modify_param(lang, param)),
                    )
                    .await?;
                }
            }
        }
        State::AwaitingModifyParam { name } => {
            // Пользователь ввёл текст вместо нажатия кнопки выбора параметра —
            // не сбрасываем диалог, переспрашиваем с подсказкой.
            bot.send_message(
                msg.chat.id,
                format!("{} {}", i18n::modify_param_select_title(lang), name),
            )
            .reply_markup(menu::modify_param_menu(lang, &name))
            .parse_mode(ParseMode::Html)
            .await?;
        }
        State::AwaitingBulkPrefix => {
            let prefix = msg.text().unwrap_or_default().to_string();
            // Худший случай сразу (count=MAX_BULK, slug по текущей настройке):
            // слишком длинный префикс отбивается на первом шаге, а не после
            // выбора срока и PSK в finish_bulk.
            let slug_enabled = false;
            match crate::vpn::validate::validate_bulk_prefix(prefix.trim(), slug_enabled) {
                Ok(()) => {
                    bot.send_message(msg.chat.id, i18n::ask_bulk_count(lang))
                        .reply_markup(menu::bulk_count_menu(lang))
                        .await?;
                    dialogue
                        .update(State::AwaitingBulkCount {
                            prefix: prefix.trim().to_string(),
                        })
                        .await?;
                }
                Err(_) => {
                    let max = crate::vpn::validate::max_bulk_prefix_len(slug_enabled);
                    bot.send_message(msg.chat.id, i18n::bad_bulk_prefix(lang, max))
                        .await?;
                }
            }
        }
        State::AwaitingBulkCustomExpiry { prefix, count } => {
            let raw = msg.text().unwrap_or_default().to_string();
            match crate::vpn::validate::validate_expiry(&raw) {
                Ok(exp) => {
                    bot.send_message(msg.chat.id, i18n::psk_step(lang, settings.psk_default()))
                        .reply_markup(menu::bulk_psk_step(lang, settings.psk_default()))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    dialogue
                        .update(State::AwaitingBulkPsk {
                            prefix,
                            count,
                            expires: Some(exp),
                        })
                        .await?;
                }
                Err(_) => {
                    bot.send_message(msg.chat.id, i18n::bad_expiry(lang))
                        .await?;
                }
            }
        }
        State::AwaitingGroupName => {
            if !role.is_owner() {
                dialogue.update(State::Idle).await?;
                return Ok(());
            }
            let raw = msg.text().unwrap_or_default().trim().to_string();
            if raw.is_empty() || raw.chars().count() > 32 {
                bot.send_message(msg.chat.id, i18n::bad_group_name(lang))
                    .await?;
            } else {
                match settings.create_group(&raw, now_epoch()) {
                    Ok(_) => {
                        settings.log_event(
                            now_epoch(),
                            EventKind::GroupCreate,
                            None,
                            Some(uid),
                            Some(&raw),
                        );
                        bot.send_message(msg.chat.id, i18n::group_created(lang, &raw))
                            .parse_mode(ParseMode::Html)
                            .reply_markup(menu::main_menu(lang))
                            .await?;
                        dialogue.update(State::Idle).await?;
                    }
                    Err(crate::store::GroupError::NameTaken) => {
                        bot.send_message(msg.chat.id, i18n::group_name_taken(lang, &raw))
                            .parse_mode(ParseMode::Html)
                            .await?;
                    }
                    // NotFound для INSERT недостижим — сворачиваем в общий сбой.
                    Err(crate::store::GroupError::Db | crate::store::GroupError::NotFound) => {
                        let err = crate::error::Error::Telegram("db".into());
                        bot.send_message(msg.chat.id, i18n::error_text(lang, &err))
                            .await?;
                        dialogue.update(State::Idle).await?;
                    }
                }
            }
        }
        State::AwaitingGroupRename { id } => {
            if !role.is_owner() {
                dialogue.update(State::Idle).await?;
                return Ok(());
            }
            let raw = msg.text().unwrap_or_default().trim().to_string();
            if raw.is_empty() || raw.chars().count() > 32 {
                bot.send_message(msg.chat.id, i18n::bad_group_name(lang))
                    .await?;
            } else {
                match settings.rename_group(id, &raw) {
                    Ok(()) => {
                        settings.log_event(
                            now_epoch(),
                            EventKind::GroupRename,
                            None,
                            Some(uid),
                            Some(&raw),
                        );
                        bot.send_message(msg.chat.id, i18n::group_renamed(lang, &raw))
                            .parse_mode(ParseMode::Html)
                            .reply_markup(menu::main_menu(lang))
                            .await?;
                        dialogue.update(State::Idle).await?;
                    }
                    Err(crate::store::GroupError::NameTaken) => {
                        bot.send_message(msg.chat.id, i18n::group_name_taken(lang, &raw))
                            .parse_mode(ParseMode::Html)
                            .await?;
                    }
                    Err(crate::store::GroupError::NotFound) => {
                        // Группу удалили, пока владелец вводил новое имя.
                        bot.send_message(msg.chat.id, i18n::not_found(lang))
                            .reply_markup(menu::main_menu(lang))
                            .await?;
                        dialogue.update(State::Idle).await?;
                    }
                    Err(crate::store::GroupError::Db) => {
                        let err = crate::error::Error::Telegram("db".into());
                        bot.send_message(msg.chat.id, i18n::error_text(lang, &err))
                            .await?;
                        dialogue.update(State::Idle).await?;
                    }
                }
            }
        }
        State::AwaitingGroupQuota { id } => {
            if !role.is_owner() {
                dialogue.update(State::Idle).await?;
                return Ok(());
            }
            let raw = msg.text().unwrap_or_default().trim().to_string();
            match raw.parse::<i64>() {
                Ok(n) if (0..=100_000).contains(&n) => {
                    let quota = if n == 0 { None } else { Some(n) };
                    if settings.set_group_quota(id, quota) {
                        settings.log_event(
                            now_epoch(),
                            EventKind::GroupQuota,
                            None,
                            Some(uid),
                            Some(&format!(
                                "group={id} quota={}",
                                quota.map_or_else(|| "unlimited".to_string(), |q| q.to_string())
                            )),
                        );
                        bot.send_message(msg.chat.id, i18n::group_quota_set(lang, quota))
                            .reply_markup(menu::main_menu(lang))
                            .parse_mode(ParseMode::Html)
                            .await?;
                    } else {
                        // Группу удалили, пока владелец вводил лимит.
                        bot.send_message(msg.chat.id, i18n::not_found(lang))
                            .reply_markup(menu::main_menu(lang))
                            .await?;
                    }
                    dialogue.update(State::Idle).await?;
                }
                _ => {
                    bot.send_message(msg.chat.id, i18n::bad_group_quota(lang))
                        .await?;
                }
            }
        }
        State::AwaitingGroupAdminId { id } => {
            if !role.is_owner() {
                dialogue.update(State::Idle).await?;
                return Ok(());
            }
            let raw = msg.text().unwrap_or_default().trim().to_string();
            match raw.parse::<i64>() {
                Ok(new_admin) if new_admin > 0 => {
                    let first_admin_ever = !settings.has_any_group_admin();
                    let gname = settings.group(id).map(|g| g.name).unwrap_or_default();
                    if settings.add_group_admin(id, new_admin, uid, now_epoch()) {
                        settings.log_event(
                            now_epoch(),
                            EventKind::AdminAdd,
                            None,
                            Some(uid),
                            Some(&format!("group={id} user={new_admin} via=manual")),
                        );
                        bot.send_message(msg.chat.id, i18n::admin_added(lang, new_admin, &gname))
                            .parse_mode(ParseMode::Html)
                            .reply_markup(menu::main_menu(lang))
                            .await?;
                        let _ = first_admin_ever;
                    } else {
                        bot.send_message(msg.chat.id, i18n::admin_already(lang, new_admin))
                            .await?;
                    }
                    dialogue.update(State::Idle).await?;
                }
                _ => {
                    bot.send_message(msg.chat.id, i18n::bad_admin_id(lang))
                        .await?;
                }
            }
        }
        State::AwaitingClientOwner { name } => {
            let raw = msg.text().unwrap_or_default().trim();
            let user = raw
                .parse::<i64>()
                .ok()
                .and_then(|id| settings.user(id))
                .or_else(|| settings.find_user_by_username(raw));
            if let Some(user) = user {
                if settings.assign_client_owner(&name, Some(user.user_id)) {
                    bot.send_message(
                        msg.chat.id,
                        format!("✅ Ключ {name} привязан к пользователю {}.", user.user_id),
                    )
                    .reply_markup(menu::main_menu(lang))
                    .await?;
                }
                dialogue.update(State::Idle).await?;
            } else {
                bot.send_message(msg.chat.id, "Пользователь не найден. Он должен сначала запустить бота. Введите Telegram ID или @username ещё раз.").await?;
            }
        }
        _ => {
            // /start и всё прочее.
            if !settings.has_lang(uid) {
                // Язык-гейт: пользователь ещё не выбрал язык — показать выбор
                // без parse_mode (choose_language() не содержит HTML-разметки).
                bot.send_message(msg.chat.id, i18n::choose_language())
                    .reply_markup(menu::language_select())
                    .await?;
            } else {
                match &role {
                    Role::GroupAdmin(groups) => match current_ga_group(&settings, uid, groups) {
                        Some(gid) => {
                            let gname = settings.group(gid).map(|g| g.name).unwrap_or_default();
                            bot.send_message(msg.chat.id, i18n::ga_menu_title(lang, &gname))
                                .reply_markup(menu::ga_main_menu(lang, groups.len() > 1))
                                .parse_mode(ParseMode::Html)
                                .await?;
                        }
                        None => {
                            let rows: Vec<_> =
                                groups.iter().filter_map(|g| settings.group(*g)).collect();
                            bot.send_message(msg.chat.id, i18n::select_group_title(lang))
                                .reply_markup(menu::group_select_menu(lang, &rows))
                                .await?;
                        }
                    },
                    _ => {
                        bot.send_message(msg.chat.id, i18n::menu_title(lang))
                            .reply_markup(menu::main_menu(lang))
                            .parse_mode(ParseMode::Html)
                            .await?;
                    }
                }
            }
            dialogue.update(State::Idle).await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn finish_add(
    bot: &Bot,
    chat: ChatId,
    vpn: &Vpn,
    settings: &Store,
    lang: Lang,
    name: &str,
    expires: Option<&str>,
    psk: bool,
    recreate: bool,
    uid: i64,
    group: Option<i64>,
    role: &Role,
    server: &crate::store::VpnServer,
) {
    let home = home_menu(role, lang);
    let waiting = bot.send_message(chat, i18n::creating(lang)).await.ok();
    // Квота группы: проверка непосредственно перед созданием. Best-effort:
    // при двух конкурентных созданиях обе проверки могут пройти до add —
    // перелёт максимум на глубину гонки, системно квота не копится. Только
    // для не-recreate: recreate удаляет старого клиента перед add и сохраняет
    // его группу (group_for_new_client), нетто-число клиентов не растёт.
    if !recreate {
        if let Some(gid) = group {
            if let Some(remaining) = settings.group_remaining(gid) {
                if remaining < 1 {
                    if let Some(m) = waiting {
                        let _ = bot.delete_message(chat, m.id).await;
                    }
                    let quota = settings.group(gid).and_then(|g| g.max_clients).unwrap_or(0);
                    let _ = bot
                        .send_message(chat, i18n::quota_reached(lang, quota))
                        .await;
                    return;
                }
            }
        }
    }
    if recreate {
        // Удаляем старого клиента перед созданием нового. Если remove упадёт —
        // не создаём нового, показываем ошибку; старый клиент остаётся.
        if let Err(e) = client_remove(vpn, settings, name).await {
            tracing::error!(error = %e, "remove перед recreate провалился");
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            let _ = bot.send_message(chat, i18n::error_text(lang, &e)).await;
            return;
        }
    }
    let creation = if server.is_local {
        vpn.add(name, expires, psk).await
    } else {
        match nonlocal_add(vpn, settings, server, name).await {
            Ok(result) => {
                if let Some(duration) = expires {
                    let expiry_result = duration_seconds(duration)
                        .ok_or_else(|| crate::error::Error::Parse("неверный срок ключа".into()));
                    let with_expiry = match expiry_result {
                        Ok(seconds) => {
                            nonlocal_set_expiry(vpn, settings, server, name, now_epoch() + seconds)
                                .await
                                .map(|_| result)
                        }
                        Err(error) => Err(error),
                    };
                    if with_expiry.is_err() {
                        let _ = nonlocal_remove(vpn, settings, server, name).await;
                    }
                    with_expiry
                } else {
                    Ok(result)
                }
            }
            Err(error) => Err(error),
        }
    };
    match creation {
        Ok(res) => {
            settings.log_event(
                now_epoch(),
                EventKind::ClientAdd,
                Some(name),
                Some(uid),
                None,
            );
            // Безусловно, а не только при Some(group): строка клиента с этим
            // именем могла остаться от ранее удалённого клиента с чужим
            // group_id (ON CONFLICT... DO UPDATE в assign_client_group её не
            // создаёт заново, а перезатирает). Без безусловного вызова при
            // group=None «воскресшая» строка сохранила бы старую привязку —
            // группа-владелец получил бы доступ к новому чужому клиенту.
            //
            // Привязка к группе. Для нового клиента с группой — атомарно с
            // квотой: ранняя проверка выше могла пройти у двух конкурентов
            // одновременно (vpn.add занимает секунды), решает только этот
            // вызов. Recreate и клиент без группы — как раньше (квота не
            // растёт / не применима); безусловность вызова при group=None
            // сохраняется — см. комментарий про «воскресшую» строку выше.
            let outcome = match group {
                Some(gid) if !recreate => settings.try_assign_client_group(name, gid, now_epoch()),
                _ => {
                    settings.assign_client_group(name, group, now_epoch());
                    crate::store::QuotaAssign::Assigned
                }
            };
            if !settings.assign_client_server(name, server.id, &server.protocol) {
                let _ = if server.is_local {
                    vpn.remove(name).await
                } else {
                    nonlocal_remove(vpn, settings, server, name).await
                };
                if let Some(m) = waiting {
                    let _ = bot.delete_message(chat, m.id).await;
                }
                let error = crate::error::Error::Parse(
                    "не удалось закрепить ключ за выбранным сервером".into(),
                );
                let _ = bot.send_message(chat, i18n::error_text(lang, &error)).await;
                return;
            }
            if add_needs_quota_rollback(recreate, &outcome) {
                // Компенсация: клиент создан, но в группу не влез — удаляем
                // его и показываем «квота исчерпана». Артефакты не выдаём:
                // клиент через мгновение перестанет существовать. В историю
                // попадают ОБА события (add выше уже залогирован) — она
                // отражает то, что реально произошло.
                let gid = group.expect("rollback только при Some(group)");
                if let Err(e) = client_remove(vpn, settings, name).await {
                    // Откат не удался: клиент существует, но без группы —
                    // виден только владельцу, чинится вручную. Пользователю —
                    // честная ошибка.
                    tracing::error!(error = %e, client = name, "не удалось откатить клиента после гонки квоты");
                    if let Some(m) = waiting {
                        let _ = bot.delete_message(chat, m.id).await;
                    }
                    let _ = bot.send_message(chat, i18n::error_text(lang, &e)).await;
                    return;
                }
                settings.log_event(
                    now_epoch(),
                    EventKind::ClientRemove,
                    Some(name),
                    Some(uid),
                    Some("quota race rollback"),
                );
                if let Some(m) = waiting {
                    let _ = bot.delete_message(chat, m.id).await;
                }
                let quota = settings.group(gid).and_then(|g| g.max_clients).unwrap_or(0);
                let _ = bot
                    .send_message(chat, i18n::quota_reached(lang, quota))
                    .reply_markup(home)
                    .await;
                return;
            }
            // Фильтр выдачи по тумблерам настроек (deliver_conf/qr/link): после
            // создания шлём только включённые артефакты. Ручная повторная выдача
            // через карточку клиента (SendConf/SendQr/SendLink/SendAll) фильтр
            // игнорирует — это явный запрос конкретного файла.
            if let Err(e) = render::send_client_files_filtered(
                bot,
                chat,
                lang,
                &res,
                settings.deliver_conf(),
                settings.deliver_qr(),
                settings.deliver_link(),
            )
            .await
            {
                tracing::error!(error = %e, "не удалось отправить файлы клиента");
                let _ = bot.send_message(chat, i18n::error_text(lang, &e)).await;
            }
        }
        // Гонка: клиент появился между проверкой exists() и add — скрипт молча
        // пропустил создание (rc 0). Показываем то же предупреждение, что и при
        // обычном совпадении имени; кнопку «Пересоздать» — только если клиент
        // в скоупе роли (как в AwaitingName: групповому админу нельзя
        // предлагать пересоздание чужого клиента).
        Err(crate::error::Error::ClientExists(_)) => {
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            let kb = if client_in_scope(role, settings, name) {
                menu::confirm_recreate(lang, name)
            } else {
                home
            };
            let _ = bot
                .send_message(chat, i18n::client_exists(lang, name))
                .reply_markup(kb)
                .parse_mode(ParseMode::Html)
                .await;
            return;
        }
        Err(e) => {
            // Клиент не создан — ранний return, чтобы общий хвост не слал
            // «Готово» следом за ошибкой (#40). Клавиатуру возвращаем: без
            // неё пользователь оставался бы без меню после сбоя.
            tracing::error!(error = %e, "add провалился");
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            let _ = bot
                .send_message(chat, i18n::error_text(lang, &e))
                .reply_markup(home)
                .await;
            return;
        }
    }
    if let Some(m) = waiting {
        let _ = bot.delete_message(chat, m.id).await;
    }
    let _ = bot
        .send_message(chat, i18n::done(lang))
        .reply_markup(home)
        .parse_mode(ParseMode::Html)
        .await;
}

/// Завершающий шаг массовой генерации: превентивные проверки (вместо длинной
/// паузы в скрипте, после которой пользователь видит «ошибка»), затем один
/// вызов `add_many` и выдача альбома .conf одним `sendMediaGroup`.
///
/// Превентивный гейт состоит из трёх проверок до запуска скрипта:
/// · **имена**: `gen_bulk_names` с актуальным slug (smoke-проверка длины/символов);
/// · **capacity**: `vpn.capacity()` — `free == 0` или `free < count` не даём
///   начинать (неинформативно падать внутри add-many-цикла);
/// · **коллизии**: `vpn.list()` ∩ сгенерированные имена — хоть add_many и
///   превратит коллизии в `Skip`, лучше подсветить это ДО создания (fail-fast),
///   чтобы пользователь мог сменить префикс. `list` fail-open (warn + continue):
///   временную недоступность check/list не превращаем в молчаливый отказ.
///
/// Сами клиенты создаются через `add_many` (один вызов скрипта, один apply_config
/// в конце). Альбом .conf шлём только если включён тумблер `deliver_conf` и есть
/// хоть один созданный клиент (пустой альбом Telegram отклонит).
#[allow(clippy::too_many_arguments)]
async fn finish_bulk(
    bot: &Bot,
    chat: ChatId,
    vpn: &Vpn,
    settings: &Store,
    lang: Lang,
    prefix: &str,
    count: usize,
    expires: Option<&str>,
    psk: bool,
    uid: i64,
    group: Option<i64>,
    server: &crate::store::VpnServer,
) {
    let waiting = bot.send_message(chat, i18n::bulk_creating(lang)).await.ok();

    // 1. Продолжаем нумерацию и заполняем свободные пропуски.
    let existing = settings
        .active_client_names()
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    let names = match crate::vpn::validate::gen_available_names(prefix, count, &existing) {
        Ok(n) => n,
        Err(_) => {
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            let max = crate::vpn::validate::max_bulk_prefix_len(false);
            let _ = bot
                .send_message(chat, i18n::bad_bulk_prefix(lang, max))
                .await;
            return;
        }
    };

    if let Some(gid) = group {
        if let Some(remaining) = settings.group_remaining(gid) {
            if remaining < count as i64 {
                if let Some(m) = waiting {
                    let _ = bot.delete_message(chat, m.id).await;
                }
                let quota = settings.group(gid).and_then(|g| g.max_clients).unwrap_or(0);
                let _ = bot
                    .send_message(chat, i18n::quota_reached(lang, quota))
                    .await;
                return;
            }
        }
    }

    if !server.is_local {
        let mut created = 0usize;
        let expiry = expires
            .and_then(duration_seconds)
            .map(|seconds| now_epoch() + seconds);
        for name in &names {
            let result = async {
                let files = nonlocal_add(vpn, settings, server, name).await?;
                if let Some(expires_at) = expiry {
                    if let Err(error) =
                        nonlocal_set_expiry(vpn, settings, server, name, expires_at).await
                    {
                        let _ = nonlocal_remove(vpn, settings, server, name).await;
                        return Err(error);
                    }
                }
                settings.assign_client_group(name, group, now_epoch());
                if !settings.assign_client_server(name, server.id, &server.protocol) {
                    let _ = nonlocal_remove(vpn, settings, server, name).await;
                    return Err(crate::error::Error::Parse(
                        "не удалось закрепить пакетный ключ за сервером".into(),
                    ));
                }
                Ok::<_, crate::error::Error>(files)
            }
            .await;
            match result {
                Ok(files) => {
                    created += 1;
                    settings.log_event(
                        now_epoch(),
                        EventKind::ClientAdd,
                        Some(name),
                        Some(uid),
                        Some("bulk_remote"),
                    );
                    if let Err(error) = render::send_client_files(bot, chat, lang, &files).await {
                        tracing::error!(%error, client = %name, "не удалось отправить пакетный ключ");
                    }
                }
                Err(error) => {
                    tracing::error!(%error, client = %name, "не удалось создать пакетный ключ");
                }
            }
        }
        if let Some(message) = waiting {
            let _ = bot.delete_message(chat, message.id).await;
        }
        let _ = bot
            .send_message(
                chat,
                format!(
                    "✅ На сервере «{}» создано {created}/{} ключей.",
                    server.location,
                    names.len()
                ),
            )
            .reply_markup(menu::main_menu(lang))
            .await;
        return;
    }

    // 2. Превентивная проверка свободных адресов (capacity учитывает v4-подсеть).
    match vpn.capacity().await {
        Ok(cap) => {
            if cap.free == 0 {
                if let Some(m) = waiting {
                    let _ = bot.delete_message(chat, m.id).await;
                }
                let _ = bot.send_message(chat, i18n::capacity_exhausted(lang)).await;
                return;
            }
            if (cap.free as usize) < count {
                if let Some(m) = waiting {
                    let _ = bot.delete_message(chat, m.id).await;
                }
                let _ = bot
                    .send_message(
                        chat,
                        i18n::capacity_insufficient(lang, cap.free, count as u32),
                    )
                    .await;
                return;
            }
        }
        Err(_) => {
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            let _ = bot
                .send_message(chat, i18n::capacity_unavailable(lang))
                .await;
            return;
        }
    }

    // 3. Один вызов add_many (сразу со всеми именами). add_many возвращает
    // BulkResult{created, skipped} — все результаты, не только первый.
    match vpn.add_many(&names, expires, psk).await {
        Ok(res) => {
            for r in &res.created {
                settings.log_event(
                    now_epoch(),
                    EventKind::ClientAdd,
                    Some(&r.name),
                    Some(uid),
                    Some("bulk"),
                );
                // Безусловно (см. finish_add): имя может быть переиспользовано
                // после удаления клиента с чужим group_id — bulk всегда
                // owner-only и без группы, поэтому явно отвязываем строку,
                // а не оставляем «воскресшую» привязку от прежнего клиента.
                settings.assign_client_group(&r.name, group, now_epoch());
                settings.assign_client_server(&r.name, server.id, &server.protocol);
            }
            // 4. Telegram принимает не больше 10 элементов в одном альбоме.
            if settings.deliver_conf() && !res.created.is_empty() {
                let conf_paths: Vec<String> =
                    res.created.iter().map(|c| c.conf_path.clone()).collect();
                for chunk in conf_paths.chunks(10) {
                    if let Err(e) = render::send_album(bot, chat, chunk).await {
                        tracing::error!(error = %e, "альбом .conf не отправлен");
                        let _ = bot.send_message(chat, i18n::error_text(lang, &e)).await;
                    }
                }
            }
            // 6. Итог: «Создано N» (+ список пропущенных с причинами, если есть).
            let _ = bot
                .send_message(chat, i18n::bulk_result_summary(lang, &res))
                .parse_mode(ParseMode::Html)
                .reply_markup(menu::main_menu(lang))
                .await;
        }
        Err(e) => {
            tracing::error!(error = %e, "add_many провалился");
            let _ = bot.send_message(chat, i18n::error_text(lang, &e)).await;
        }
    }
    if let Some(m) = waiting {
        let _ = bot.delete_message(chat, m.id).await;
    }
}

/// Экран пустого списка клиентов. Клиентов нет вообще (`total == 0`) —
/// «Пока нет клиентов» + домашняя клавиатура. Пусто из-за фильтра/скоупа —
/// текст про фильтр + клавиатура с кнопками смены фильтра и скоупа: липкие
/// настройки иначе делают раздел недоступным (#20 — «Без группы» при
/// полностью распределённых клиентах).
fn empty_list_screen(
    lang: Lang,
    total: usize,
    filter: crate::vpn::model::ClientFilter,
    is_owner: bool,
    home: InlineKeyboardMarkup,
) -> (String, InlineKeyboardMarkup) {
    if total == 0 {
        (i18n::clients_empty(lang), home)
    } else {
        (
            i18n::clients_empty_filtered(lang),
            menu::clients_empty_menu(lang, filter, is_owner),
        )
    }
}

/// Рендерит экран списка клиентов: list_enriched → filter+sort → expiries →
/// title → clients_list. Общая логика для Action::List / Action::Page /
/// Action::SetListFilter (различаются только страницей и тем, кто читает/
/// устанавливает фильтр из настроек). Пустой список — empty_list_screen.
#[allow(clippy::too_many_arguments)]
async fn render_clients_list(
    bot: &Bot,
    chat: ChatId,
    msg_id: MessageId,
    lang: Lang,
    vpn: &Vpn,
    settings: &Store,
    uid: i64,
    page: usize,
    scope: ListScope,
    home: InlineKeyboardMarkup,
    is_owner: bool,
) {
    // list_enriched = status_code из list (корректная трёхцветная классификация)
    // + last_handshake/rx/tx из stats (метка времени для кнопки). Чистый stats
    // (#27) терял жёлтый статус никогда не подключавшихся клиентов (inactive 🔴
    // вместо no_handshake 🟡) — см. vpn::Vpn::list_enriched.
    match managed_clients(vpn, settings).await {
        Ok(all_clients) => {
            // Фильтр + сортировка «онлайн вперёд» (🟢 → 🔴 → 🟡, внутри — по имени).
            // apply_filter_and_sort возвращает owned Vec — clients_list берёт срез по странице.
            let filter = settings.client_filter(uid);
            let clients =
                crate::vpn::model::apply_filter_and_sort(&all_clients, filter, now_epoch());
            // Скоуп: групповому админу — его текущая группа; владельцу — выбранный
            // фильтр группы (Task 13) или все.
            let clients: Vec<_> = clients
                .into_iter()
                .filter(|c| scope.admits(settings.client_group(&c.name)))
                .collect();
            if clients.is_empty() {
                let (text, kb) = empty_list_screen(lang, all_clients.len(), filter, is_owner, home);
                edit_or_send(bot, chat, msg_id, text, kb).await;
                return;
            }
            // Полный вектор (не страница): clients_list индексирует expiries[i]
            // по глобальному i, срез по странице дал бы сдвиг меток на страницах > 0.
            let expiries: Vec<Option<i64>> =
                clients.iter().map(|c| vpn.client_expiry(&c.name)).collect();
            let title =
                i18n::clients_title_filtered(lang, filter, clients.len(), all_clients.len());
            edit_or_send(
                bot,
                chat,
                msg_id,
                title,
                menu::clients_list(
                    lang,
                    &clients,
                    &expiries,
                    now_epoch(),
                    page,
                    8,
                    filter,
                    is_owner,
                ),
            )
            .await;
        }
        Err(e) => {
            tracing::error!(error = %e, "stats провалился");
            let _ = bot.send_message(chat, i18n::error_text(lang, &e)).await;
        }
    }
}

async fn callback_handler(
    bot: Bot,
    dialogue: MyDialogue,
    q: CallbackQuery,
    cfg: Arc<Config>,
    vpn: Arc<Vpn>,
    settings: Arc<Store>,
) -> HandlerResult {
    bot.answer_callback_query(q.id.clone()).await.ok();

    let src = match &q.message {
        Some(m) => m,
        None => return Ok(()),
    };
    if !src.chat().is_private() {
        // Секреты (конфиги, QR, ссылки, бэкапы, диагностика) уходят в чат
        // апдейта — в группе они утекли бы всем участникам. Callback уже
        // отвечен выше, тут просто молча отказываем без запуска VPN-действий.
        return Ok(());
    }
    let chat = src.chat().id;
    // Сообщение-источник кнопки: навигация (меню/список/страница) редактирует
    // его на месте через edit_or_send, а не отправляет новое — так меню↔список
    // живут в одном сообщении без спама и без глобального хранилища message_id.
    let msg_id = src.id();

    let uid = user_id_of_cb(&q);
    settings.upsert_user(
        uid,
        q.from.username.as_deref(),
        &q.from.full_name(),
        None,
        now_epoch(),
    );
    let role = resolve_role(uid, &cfg.admin_ids, &settings);
    let data = q.data.clone().unwrap_or_default();
    let action = parse_callback(&data);
    let customer_action = matches!(
        &action,
        Action::Buy
            | Action::BuyServer(_)
            | Action::BuyTerm(_)
            | Action::BuyMethod(_, _)
            | Action::BuyPaid(_)
            | Action::MyKeys
            | Action::Profile
            | Action::Portal
            | Action::Balance
            | Action::CustomerKey(_)
            | Action::CustomerMove(_)
            | Action::CustomerMoveServer(_, _)
            | Action::CustomerMoveConfirm(_)
            | Action::CustomerMoveCancel(_)
            | Action::CustomerRefresh(_)
            | Action::CustomerRefreshRun(_)
            | Action::Renew(_)
            | Action::RenewTerm(_, _)
            | Action::RenewMethod(_, _, _)
            | Action::LegacyRenew(_)
            | Action::LegacyRenewMethod(_, _)
            | Action::LegacyRequestNew
            | Action::PromoInput
            | Action::Guide(_)
            | Action::AutoRenew(_, _, _)
            | Action::DeviceLabelAsk(_)
            | Action::SupportNewCategory(_)
            | Action::SupportRate(_, _)
    );
    if role == Role::Denied
        && settings.user_blocked(uid)
        && !matches!(
            &action,
            Action::SupportNewCategory(_) | Action::SupportRate(_, _)
        )
    {
        bot.send_message(
            chat,
            "⛔ Доступ к боту приостановлен. Обратитесь в поддержку.",
        )
        .await?;
        return Ok(());
    }
    if role == Role::Denied && !customer_action {
        tracing::warn!(user_id = uid, "отклонён доступ (callback)");
        return Ok(());
    }
    let lang = settings.lang(uid);
    // Единая авторизация. Отказ — молчаливый выход: callback уже отвечен в
    // начале функции, прежние guard'ы вели себя так же.
    if role != Role::Denied && !authorize(&action, &role, &settings) {
        return Ok(());
    }
    match action {
        Action::AdminDashboard => {
            admin_dashboard(&bot, chat, &vpn, &settings).await?;
        }
        Action::AdminVpn => {
            bot.send_message(chat, "🛡 Управление VPN-службой\n\nПроверка показывает состояние systemd-юнита, интерфейса, порта, маршрутизации, firewall и клиентов. Диагностика формирует подробный технический отчёт.\n\nПерезапуск кратковременно разорвёт активные VPN-соединения и требует подтверждения.")
                .reply_markup(menu::vpn_service_menu())
                .await?;
        }
        Action::AdminServers => {
            servers_screen(&bot, chat, &settings).await?;
        }
        Action::AdminKeys => {
            bot.send_message(
                chat,
                "🔑 Ключи\n\nСоздание, владельцы, группы, восстановление и массовое управление.",
            )
            .reply_markup(menu::admin_keys_hub())
            .await?;
        }
        Action::AdminUsersHub => {
            bot.send_message(
                chat,
                "👥 Пользователи\n\nCRM-карточки, поиск и роли сотрудников.",
            )
            .reply_markup(menu::admin_users_hub())
            .await?;
        }
        Action::AdminCommunication => {
            bot.send_message(
                chat,
                "💬 Связь\n\nТехническая поддержка и рассылки пользователям.",
            )
            .reply_markup(menu::admin_communication_hub())
            .await?;
        }
        Action::AdminSystem => {
            let current = format!("v{}", env!("CARGO_PKG_VERSION"));
            let release = latest_release_info().await;
            let update_line = match &release {
                Some((latest, _, _)) if latest == &current => {
                    format!("✅ Установлена актуальная версия {current}")
                }
                Some((latest, _, _)) => {
                    format!("⬆️ Доступно обновление: {current} → {latest}")
                }
                None => format!("Текущая версия: {current}\nНе удалось проверить GitHub."),
            };
            bot.send_message(
                chat,
                format!("⚙️ Система\n\n{update_line}\n\nНастройки, резервные копии, VPN-служба и журнал обновлений."),
            )
            .reply_markup(menu::admin_system_hub())
            .await?;
        }
        Action::AdminUpdate => {
            let current = format!("v{}", env!("CARGO_PKG_VERSION"));
            let details = match latest_release_info().await {
                Some((latest, body, url)) => format!("Текущая версия: {current}\nДоступная версия: {latest}\n\nИзменения:\n{body}\n\n{url}"),
                None => format!("Текущая версия: {current}\n\nНе удалось получить описание релиза, но можно установить последний стабильный выпуск."),
            };
            bot.send_message(chat, format!("⬆️ Обновление бота\n\n{details}\n\nУстановить обновление? Служба автоматически перезапустится; VPN-соединения и ключи не затрагиваются."))
                .reply_markup(menu::bot_update_confirm_menu())
                .await?;
        }
        Action::AdminUpdateRun => {
            bot.send_message(chat, "⏳ Обновление запущено. Бот вернётся после автоматического перезапуска. Статус: systemctl status awgram-self-update")
                .await?;
            if let Err(error) = vpn.schedule_bot_update().await {
                bot.send_message(chat, format!("❌ Не удалось запустить обновление: {error}"))
                    .reply_markup(menu::admin_system_hub())
                    .await?;
            }
        }
        Action::AdminUpdateStatus => {
            let text = match vpn.bot_update_control("status").await {
                Ok(output) => format!("📋 Журнал обновления\n\n{}", truncate_for_message(output)),
                Err(error) => format!("❌ Не удалось прочитать журнал обновления: {error}"),
            };
            bot.send_message(chat, text)
                .reply_markup(menu::bot_update_status_menu())
                .await?;
        }
        Action::AdminUpdateRollback => {
            let text = match vpn.bot_update_control("rollback").await {
                Ok(_) => "✅ Выполнен откат к предыдущему бинарнику. Служба успешно запущена."
                    .to_string(),
                Err(error) => format!("❌ Откат не выполнен: {error}"),
            };
            bot.send_message(chat, text)
                .reply_markup(menu::admin_system_hub())
                .await?;
        }
        Action::ServerAdd => {
            bot.send_message(chat,"🧭 Мастер подключения нового сервера\n\nШаг 1 из 3. Отправьте понятное название сервера, например: Netherlands 3.0\n\nПосле создания мастер предложит автоматическую установку AWG 1.0, подключение панели или безопасную bootstrap-команду.").await?;
            dialogue.update(State::AwaitingServerWizardName).await?;
        }
        Action::ServerCard(id) => {
            if let Some(server) = settings.vpn_server(id) {
                bot.send_message(chat, server_card_text(&server, now_epoch()))
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
            }
        }
        Action::RemoteMigration(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                bot.send_message(chat, format!("🔀 Миграция «{}»: AWG 2.0 → AWG 1.0\n\nIP: {}\nОперация создаст резервную копию, остановит VPN и может дважды перезагрузить только выбранный сервер. Старые пользовательские конфигурации перестанут работать.\n\nСначала выполните предварительную проверку.", server.name, server.public_ip))
                    .reply_markup(menu::remote_migration_menu(id)).await?;
            }
        }
        Action::RemoteMigrationPreflight(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                let text = match vpn.remote_legacy_migration(&server, "preflight").await {
                    Ok(output) => format!("✅ Сервер готов к миграции.\n\n{output}"),
                    Err(error) => format!("❌ Миграцию запускать нельзя: {error}"),
                };
                bot.send_message(chat, text)
                    .reply_markup(menu::remote_migration_menu(id))
                    .await?;
            }
        }
        Action::RemoteMigrationStatus(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                let text = match vpn.remote_legacy_migration(&server, "status").await {
                    Ok(output) => format!("📍 Статус миграции «{}»\n\n{output}", server.name),
                    Err(error) => format!("❌ Статус недоступен: {error}\n\nЕсли VPS перезагружается, повторите через 2–3 минуты."),
                };
                bot.send_message(chat, text)
                    .reply_markup(menu::remote_migration_menu(id))
                    .await?;
            }
        }
        Action::RemoteMigrationTest(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                let status = vpn.remote_legacy_migration(&server, "status").await;
                if !status
                    .as_ref()
                    .is_ok_and(|output| output.contains("\"status\":\"complete\""))
                {
                    let details = status.unwrap_or_else(|error| error.to_string());
                    bot.send_message(chat, format!("⏳ Тестовый конфиг пока недоступен: миграция не завершена.\n\n{details}"))
                        .reply_markup(menu::remote_migration_menu(id))
                        .await?;
                } else if let Some(name) = settings.server_client_names(id).into_iter().next() {
                    match vpn.remote_existing_files(&server, &name).await {
                        Ok(result) => {
                            bot.send_message(chat, format!("🧪 Тестовый ключ «{name}» с сервера «{}».\n\nИмпортируйте новый профиль отдельно и проверьте внешний IP, DNS, сайты, Wi‑Fi и мобильную сеть. До подтверждения сервер остаётся в maintenance.", server.name)).await?;
                            if let Err(error) =
                                render::send_client_files(&bot, chat, lang, &result).await
                            {
                                bot.send_message(chat, i18n::error_text(lang, &error))
                                    .await?;
                            }
                        }
                        Err(error) => {
                            bot.send_message(
                                chat,
                                format!("❌ Не удалось получить тестовый конфиг «{name}»: {error}"),
                            )
                            .reply_markup(menu::remote_migration_menu(id))
                            .await?;
                        }
                    }
                } else {
                    bot.send_message(chat, "На этом сервере нет привязанных ключей для теста.")
                        .reply_markup(menu::remote_migration_menu(id))
                        .await?;
                }
            }
        }
        Action::RemoteMigrationApprove(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                let complete = vpn
                    .remote_legacy_migration(&server, "status")
                    .await
                    .is_ok_and(|output| output.contains("\"status\":\"complete\""));
                let healthy = vpn.remote_status(&server).await.unwrap_or(false);
                if complete && healthy && settings.approve_server_legacy_migration(id, now_epoch())
                {
                    settings.log_event(
                        now_epoch(),
                        EventKind::Migration,
                        None,
                        Some(uid),
                        Some(&format!("remote migration approved server={id}")),
                    );
                    bot.send_message(chat, format!("✅ AWG 1.0 на «{}» подтверждена. Сервер включён для новых ключей, но не назначен основным. Рассылка пользователям не запускалась.", server.name))
                        .reply_markup(menu::server_card_menu(id))
                        .await?;
                } else {
                    bot.send_message(chat, "❌ Подтверждение отклонено: миграция ещё не завершена, VPN не отвечает или сервер уже вышел из maintenance.")
                        .reply_markup(menu::remote_migration_menu(id))
                        .await?;
                }
            }
        }
        Action::RemoteMigrationAsk(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                bot.send_message(chat, format!("🚨 Последнее предупреждение\n\nБудет мигрирован только сервер «{}» ({}). VPN временно отключится, старые конфигурации клиентов станут недействительными. Убедитесь, что snapshot VPS и доступ к веб-консоли готовы.", server.name, server.public_ip))
                    .reply_markup(menu::remote_migration_confirm_menu(id)).await?;
            }
        }
        Action::RemoteMigrationRun(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                settings.set_server_status(id, "maintenance", now_epoch());
                settings.set_server_provisioning(id, false, now_epoch());
                let text = match vpn.remote_legacy_migration(&server, "start").await {
                    Ok(output) => {
                        settings.log_event(
                            now_epoch(),
                            EventKind::Migration,
                            None,
                            Some(uid),
                            Some(&format!("remote migration server={id}")),
                        );
                        format!("🚨 Миграция запущена. Сервер может дважды перезагрузиться. Не запускайте её повторно; проверяйте кнопку «Статус».\n\n{output}")
                    }
                    Err(error) => {
                        settings.set_server_status(id, &server.status, now_epoch());
                        settings.set_server_provisioning(
                            id,
                            server.enabled_for_provisioning,
                            now_epoch(),
                        );
                        format!("❌ Не удалось запустить миграцию: {error}")
                    }
                };
                bot.send_message(chat, text)
                    .reply_markup(menu::remote_migration_menu(id))
                    .await?;
            }
        }
        Action::RemoteMigrationRollback(id) => {
            if let Some(server) = settings.vpn_server(id).filter(|server| !server.is_local) {
                let text = match vpn.remote_legacy_migration(&server, "rollback").await {
                    Ok(output) => {
                        settings.finish_server_legacy_rollback(id, now_epoch());
                        format!("✅ AWG 2.0 восстановлена. Сервер снова включён для выдачи; старые пользовательские конфигурации должны работать.\n\n{output}")
                    }
                    Err(error) => format!("❌ Откат не выполнен: {error}"),
                };
                bot.send_message(chat, text)
                    .reply_markup(menu::remote_migration_menu(id))
                    .await?;
            }
        }
        Action::ServerBilling => {
            let servers = settings.vpn_servers();
            let lines = servers
                .iter()
                .map(|s| {
                    format!(
                        "• {} — {}",
                        s.name,
                        s.paid_until
                            .map(crate::calendar::format_date)
                            .unwrap_or_else(|| "не настроено".into())
                    )
                })
                .collect::<Vec<_>>();
            bot.send_message(
                chat,
                format!(
                    "💳 Календарь оплаты серверов\n\n{}",
                    if lines.is_empty() {
                        "Серверы ещё не добавлены".into()
                    } else {
                        lines.join("\n")
                    }
                ),
            )
            .reply_markup(menu::servers_menu(&servers))
            .await?;
        }
        Action::ServerBillingAsk(id) => {
            if settings.vpn_server(id).is_some() {
                bot.send_message(chat,"Введите:\nОПЛАЧЕН ДО | ПЕРИОД В МЕСЯЦАХ | СТОИМОСТЬ | ВАЛЮТА | АВТОПРОДЛЕНИЕ\n\nПример: 2026-09-15 | 1 | 6.00 | EUR | да").await?;
                dialogue
                    .update(State::AwaitingServerBilling { server_id: id })
                    .await?;
            }
        }
        Action::ServerPassportAsk(id) => {
            if let Some(server) = settings.vpn_server(id) {
                bot.send_message(chat,format!("✏️ Редактирование паспорта\n\nОтправьте:\nНАЗВАНИЕ | HOSTNAME | IP | ХОСТЕР | ЛОКАЦИЯ | ПРОТОКОЛ | ДАТА ОТКРЫТИЯ\n\nПоддерживаются только: amneziawg-2, amneziawg-1 и amneziawg-panel.\n\nТекущие данные:\n{} | {} | {} | {} | {} | {} | {}",server.name,server.hostname,server.public_ip,server.provider,server.location,server.protocol,server.opened_at.map(crate::calendar::format_date).unwrap_or_else(||"YYYY-MM-DD".into()))).await?;
                dialogue
                    .update(State::AwaitingServerPassport { server_id: id })
                    .await?;
            }
        }
        Action::ServerEnroll(id) => {
            if let Some(server) = settings.vpn_server(id) {
                if server.is_local {
                    bot.send_message(
                        chat,
                        "🏠 Локальный сервер уже подключён напрямую к боту и не требует bootstrap.",
                    )
                    .await?;
                } else if let (Some(node), Ok((clear_secret, encrypted_secret)), Ok(key)) = (
                    settings.vpn_node_for_server(id),
                    vpn.create_node_secret(),
                    vpn.controller_public_key_b64(),
                ) {
                    if !settings.set_node_secret(id, &encrypted_secret, now_epoch()) {
                        bot.send_message(chat, "❌ Не удалось сохранить учётные данные узла.")
                            .await?;
                        return Ok(());
                    }
                    let protocol = crate::vpn::driver::Protocol::parse(&server.protocol)
                        .map(crate::vpn::driver::Protocol::canonical)
                        .unwrap_or("amneziawg-1");
                    let command = format!("curl -fsSL https://github.com/stevefoxru/awgram/releases/latest/download/node-bootstrap.sh | sudo bash -s -- --server-id {} --node-id {} --protocol {} --controller-key-b64 {} --node-secret-b64 {}",server.id,node.id,protocol,key,clear_secret);
                    bot.send_message(chat,format!("🔐 Подключение автономного узла\n\nЗапустите команду от root на VPS {} ({}). Она установит ограниченный агент; произвольная shell-команда контроллеру недоступна. Секрет подписания показывается только внутри этой команды, в базе он хранится зашифрованным. Повторное создание команды заменяет прежний секрет.\n\n{}",server.name,server.public_ip,command)).reply_markup(menu::server_card_menu(id)).await?;
                } else {
                    bot.send_message(
                        chat,
                        "❌ Не удалось подготовить подключение узла. Проверьте ключ контроллера.",
                    )
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
                }
            }
        }
        Action::ServerEnrollRevoke(id) => {
            let revoked = settings.revoke_server_enrollments(id, now_epoch());
            bot.send_message(
                chat,
                if revoked {
                    "✅ Активное приглашение подключения отозвано."
                } else {
                    "Активных приглашений для этого сервера нет."
                },
            )
            .reply_markup(menu::server_card_menu(id))
            .await?;
        }
        Action::ServerSetDefault(id) => {
            let ready = settings
                .vpn_server(id)
                .is_some_and(|server| server.status == "online" && server.enabled_for_provisioning);
            if ready {
                settings.set_default_vpn_server(id);
                bot.send_message(chat, "✅ Сервер назначен источником новых ключей и безопасной замены нерабочих ключей.\n\nПробные ключи теперь также будут выпускаться на нём. При замене старый ключ удаляется только после подтверждения пользователя.")
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
            } else {
                bot.send_message(chat, "Сначала сервер должен подключиться, пройти проверку и быть включён для выдачи.")
                    .reply_markup(menu::server_card_menu(id)).await?;
            }
        }
        Action::ServerDeployAsk(id) => {
            if let Some(server) = settings.vpn_server(id) {
                if server.is_local {
                    bot.send_message(
                        chat,
                        "Этот сервер локальный — удалённая установка ему не нужна.",
                    )
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
                } else if !matches!(server.protocol.as_str(), "legacy" | "amneziawg-1") {
                    bot.send_message(chat, "Автоустановка поддерживает только AWG 1.0. Укажите протокол amneziawg-1 в паспорте VPS.")
                        .reply_markup(menu::server_card_menu(id)).await?;
                } else {
                    bot.send_message(chat, format!("🚀 Установка AWG 1.0 на {}\n\nОтправьте одним сообщением:\nroot | ПАРОЛЬ\n\nСообщение будет сразу удалено, пароль не сохраняется и используется только для первичного входа. После этого бот установит отдельный SSH-ключ.", server.public_ip)).await?;
                    dialogue
                        .update(State::AwaitingServerDeployCredentials { server_id: id })
                        .await?;
                }
            }
        }
        Action::ServerCheck(id) => {
            if let Some(server) = settings.vpn_server(id) {
                let result = if server.is_local {
                    vpn.check().await.map(|report| report.ok)
                } else if server.protocol == "amneziawg-panel" {
                    match settings.panel_password(id) {
                        Some(secret) => vpn.panel_clients(&server, &secret).await.map(|_| true),
                        None => Err(crate::error::Error::Parse(
                            "пароль панели не настроен".into(),
                        )),
                    }
                } else if let (Some(node), Some(secret)) =
                    (settings.vpn_node_for_server(id), settings.node_secret(id))
                {
                    vpn.agent_status(&server, &node, &secret).await
                } else {
                    vpn.remote_status(&server).await
                };
                let new_status = match &result {
                    Ok(true) => "online",
                    Ok(false) => "warning",
                    Err(_) => "offline",
                };
                settings.set_server_status(id, new_status, now_epoch());
                if new_status == "offline" {
                    settings.set_server_provisioning(id, false, now_epoch());
                }
                let affected = settings.server_client_count(id);
                let text = match result {
                    Ok(true) => format!("✅ «{}» доступен, VPN-служба активна.", server.name),
                    Ok(false) => format!("⚠️ «{}» доступен, но VPN-служба не готова.", server.name),
                    Err(error) => format!("❌ «{}» недоступен: {error}\n\n{affected} активных ключей этого сервера отмечены как нерабочие. Их владельцы увидят кнопку безопасной замены.", server.name),
                };
                bot.send_message(chat, text)
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
            }
        }
        Action::ServerDiagnose(id) => {
            if let Some(server) = settings.vpn_server(id) {
                let result = if server.is_local {
                    vpn.diagnose().await
                } else if server.protocol == "amneziawg-panel" {
                    match settings.panel_password(id) {
                        Some(secret) => vpn
                            .panel_clients(&server, &secret)
                            .await
                            .map(|clients| format!("panel=online\nclients={}", clients.len())),
                        None => Err(crate::error::Error::Parse(
                            "пароль панели не настроен".into(),
                        )),
                    }
                } else if let (Some(node), Some(secret)) =
                    (settings.vpn_node_for_server(id), settings.node_secret(id))
                {
                    vpn.agent_diagnose(&server, &node, &secret).await
                } else {
                    vpn.remote_diagnose(&server).await
                };
                let text = match result {
                    Ok(output) => format!("🔬 Диагностика «{}»\n\n{}", server.name, truncate_for_message(output)),
                    Err(error) => format!("❌ Диагностика «{}» недоступна: {error}\n\nЕсли узел подключён давно, обновите SSH-мост командой transfer.sh bridge.", server.name),
                };
                bot.send_message(chat, text)
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
            }
        }
        Action::ServerProvisioningProbe(id) => {
            if let Some(server) = settings.vpn_server(id) {
                let probe_name = format!("awgram-probe-{id}-{}", now_epoch());
                bot.send_message(
                    chat,
                    format!(
                        "🧪 Проверяю полный цикл выдачи на «{}»: создание временного ключа, получение конфигурации и обязательное удаление.",
                        server.name
                    ),
                )
                .await?;
                let created = if server.is_local {
                    vpn.add(&probe_name, Some("1d"), false).await
                } else {
                    nonlocal_add(&vpn, &settings, &server, &probe_name).await
                };
                let result = match created {
                    Ok(artifacts) => {
                        let artifact_ok = std::path::Path::new(&artifacts.conf_path).exists()
                            && std::fs::metadata(&artifacts.conf_path)
                                .map(|metadata| metadata.len() > 0)
                                .unwrap_or(false);
                        let cleanup = if server.is_local {
                            vpn.remove(&probe_name).await
                        } else {
                            nonlocal_remove(&vpn, &settings, &server, &probe_name).await
                        };
                        match (artifact_ok, cleanup) {
                            (true, Ok(())) => Ok(()),
                            (false, Ok(())) => Err(crate::error::Error::Parse(
                                "сервер создал ключ, но вернул пустую конфигурацию; временный ключ удалён"
                                    .into(),
                            )),
                            (_, Err(error)) => Err(crate::error::Error::Parse(format!(
                                "тестовый ключ создан, но автоматическая очистка не выполнена: {error}. Удалите {probe_name} вручную"
                            ))),
                        }
                    }
                    Err(error) => Err(error),
                };
                let (status, text) = match result {
                    Ok(()) => (
                        "online",
                        format!(
                            "✅ «{}» прошёл полную проверку выдачи. Тестовый ключ создан, конфигурация получена, ключ удалён.",
                            server.name
                        ),
                    ),
                    Err(error) => (
                        "warning",
                        format!("❌ «{}» не прошёл проверку выдачи:\n\n{error}", server.name),
                    ),
                };
                settings.set_server_status(id, status, now_epoch());
                bot.send_message(chat, text)
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
            }
        }
        Action::ServerPanelConnect(id) => {
            if let Some(server) = settings.vpn_server(id) {
                if server.is_local {
                    bot.send_message(chat, "Панель можно подключить только к удалённому серверу.")
                        .reply_markup(menu::server_card_menu(id))
                        .await?;
                } else {
                    bot.send_message(chat, format!("🔐 Подключение панели для «{}»\n\nОтправьте одним сообщением:\nURL | ПАРОЛЬ\n\nПример: http://panel.example:1240 | пароль\n\nСообщение будет сразу удалено. В базе сохранится только зашифрованный пароль.", server.name)).await?;
                    dialogue
                        .update(State::AwaitingPanelCredentials { server_id: id })
                        .await?;
                }
            }
        }
        Action::ServerPanelSync(id) => {
            if let Some(server) = settings
                .vpn_server(id)
                .filter(|server| server.protocol == "amneziawg-panel")
            {
                let result = match settings.panel_password(id) {
                    Some(secret) => vpn.panel_clients(&server, &secret).await,
                    None => Err(crate::error::Error::Parse(
                        "пароль панели не настроен".into(),
                    )),
                };
                match result {
                    Ok(clients) => {
                        let synced = settings.sync_panel_clients(
                            id,
                            &clients
                                .iter()
                                .map(|client| (client.name.clone(), client.address.clone()))
                                .collect::<Vec<_>>(),
                            now_epoch(),
                        );
                        settings.ingest_panel(
                            id,
                            now_epoch(),
                            &clients
                                .iter()
                                .map(|client| crate::store::Sample {
                                    name: client.name.clone(),
                                    ip: client.address.clone(),
                                    rx: client.transfer_rx,
                                    tx: client.transfer_tx,
                                    last_handshake: client.last_handshake_epoch(),
                                })
                                .collect::<Vec<_>>(),
                        );
                        for client in &clients {
                            let expiry = client
                                .expired_at
                                .as_deref()
                                .and_then(|value| value.get(..10))
                                .and_then(crate::calendar::parse_date);
                            if let Err(error) = vpn.cache_client_expiry(&client.name, expiry) {
                                tracing::warn!(%error, client = %client.name, "не удалось сохранить срок ключа панели");
                            }
                        }
                        bot.send_message(chat, format!("✅ Синхронизация завершена. В панели: {}; обновлено в боте: {synced}.\n\nНовые импортированные ключи не получают владельца автоматически — назначьте его в карточке ключа.", clients.len()))
                            .reply_markup(menu::server_card_menu(id))
                            .await?;
                    }
                    Err(error) => {
                        bot.send_message(
                            chat,
                            format!("❌ Синхронизация панели не удалась: {error}"),
                        )
                        .reply_markup(menu::server_card_menu(id))
                        .await?;
                    }
                }
            } else {
                bot.send_message(chat, "Сначала подключите панель к этому серверу.")
                    .reply_markup(menu::server_card_menu(id))
                    .await?;
            }
        }
        Action::ServerPanelAudit(id) => {
            if let Some(server) = settings
                .vpn_server(id)
                .filter(|server| server.protocol == "amneziawg-panel")
            {
                let result = match settings.panel_password(id) {
                    Some(secret) => vpn.panel_clients(&server, &secret).await,
                    None => Err(crate::error::Error::Parse(
                        "пароль панели не настроен".into(),
                    )),
                };
                match result {
                    Ok(clients) => {
                        let items = clients
                            .iter()
                            .map(|client| crate::store::InventoryItem {
                                remote_id: client.id.clone(),
                                name: client.name.clone(),
                                enabled: client.enabled,
                                rx: client.transfer_rx,
                                tx: client.transfer_tx,
                                last_handshake: client.last_handshake_epoch(),
                            })
                            .collect::<Vec<_>>();
                        let report = settings.reconcile_inventory(id, now_epoch(), &items);
                        let names = |values: &[String]| {
                            if values.is_empty() {
                                "—".into()
                            } else {
                                values
                                    .iter()
                                    .take(20)
                                    .cloned()
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            }
                        };
                        bot.send_message(chat, format!(
                            "🧾 Сверка реестра · {}\n\nВ панели: {}\nСовпало с базой: {}\n\nТолько в панели: {}\n{}\n\nТолько в базе: {}\n{}\n\nЗаписаны за другим сервером: {}\n{}\n\nДубли на нескольких серверах: {}\n{}\n\nНичего не удалено. Несоответствия сохранены для последующего решения.",
                            server.name,report.observed,report.matched,
                            report.panel_only.len(),names(&report.panel_only),
                            report.database_only.len(),names(&report.database_only),
                            report.wrong_server.len(),names(&report.wrong_server),
                            report.duplicates.len(),names(&report.duplicates)
                        )).reply_markup(menu::server_card_menu(id)).await?;
                    }
                    Err(error) => {
                        bot.send_message(
                            chat,
                            format!("❌ Не удалось сверить реестр «{}»: {error}", server.name),
                        )
                        .reply_markup(menu::server_card_menu(id))
                        .await?;
                    }
                }
            }
        }
        Action::LocalMigration => {
            bot.send_message(chat, "🔀 Миграция локального сервера: AWG 2.0 → AWG 1.0\n\nОперация перевыпустит все VPN-конфигурации, временно остановит VPN и дважды перезагрузит сервер. Владельцы, группы и сроки сохраняются по прежним именам ключей. Старые пользовательские конфигурации перестанут работать.\n\nСначала обязательно выполните предварительную проверку.")
                .reply_markup(menu::local_migration_menu()).await?;
        }
        Action::LocalMigrationPreflight => {
            match vpn.local_legacy_migration("preflight").await {
                Ok(output) => {
                    bot.send_message(
                        chat,
                        format!("✅ Предварительная проверка пройдена.\n\n{output}"),
                    )
                    .reply_markup(menu::local_migration_menu())
                    .await?
                }
                Err(error) => {
                    bot.send_message(chat, format!("❌ Миграцию начинать нельзя: {error}"))
                        .reply_markup(menu::local_migration_menu())
                        .await?
                }
            };
        }
        Action::LocalMigrationStart => {
            bot.send_message(chat, "🚨 Последнее предупреждение\n\nБудет создан системный backup, затем действующая AWG 2.0 остановится. Сервер перезагрузится до двух раз, а все пользователи должны будут получить новые конфигурации.\n\nДля запуска отправьте отдельным сообщением точно:\nMIGRATE AWG1")
                .await?;
            dialogue
                .update(State::AwaitingLocalMigrationConfirm {
                    operation: "start".into(),
                })
                .await?;
        }
        Action::LocalMigrationStatus => {
            match vpn.local_legacy_migration("status").await {
                Ok(output) => {
                    bot.send_message(chat, format!("📍 Состояние локальной миграции\n\n{output}"))
                        .reply_markup(menu::local_migration_menu())
                        .await?
                }
                Err(error) => {
                    bot.send_message(chat, format!("❌ Не удалось получить статус: {error}"))
                        .reply_markup(menu::local_migration_menu())
                        .await?
                }
            };
        }
        Action::LocalMigrationRollback => {
            bot.send_message(chat, "↩️ Аварийный откат остановит текущую AWG 1.0 и восстановит сохранённые каталоги AWG 2.0. Используйте его только если миграция завершилась ошибкой.\n\nДля отката отправьте точно:\nROLLBACK AWG2")
                .await?;
            dialogue
                .update(State::AwaitingLocalMigrationConfirm {
                    operation: "rollback".into(),
                })
                .await?;
        }
        Action::AdminCreate => {
            bot.send_message(chat,"➕ Создание ключей\n\nВыберите одиночное создание или пакет. После создания ключ можно привязать к пользователю и группе из его карточки.").reply_markup(menu::admin_create_menu()).await?;
        }
        Action::AdminOwners => {
            owners_screen(&bot, chat, &settings, 0).await?;
        }
        Action::AdminOwnersPage(page) => {
            owners_screen(&bot, chat, &settings, page).await?;
        }
        Action::AdminFinance => {
            finance_screen(&bot, chat, &settings).await?;
        }
        Action::AdminSupport => {
            support_screen(&bot, chat, &settings).await?;
        }
        Action::SupportFilter(status) => {
            if matches!(status.as_str(), "open" | "in_progress" | "closed") {
                support_filtered_screen(&bot, chat, &settings, &status).await?;
            }
        }
        Action::AdminBroadcast => {
            bot.send_message(chat, "Выберите получателей рассылки:")
                .reply_markup(menu::broadcast_audience_menu())
                .await?;
        }
        Action::AdminBroadcastTemplates => {
            bot.send_message(chat, "📝 Шаблоны рассылок\n\nНажмите на текст сообщения, чтобы быстро выделить и скопировать. Перед отправкой замените значения в {фигурных скобках}.\n\n<pre>⚠️ Технические работы\n\n{дата} с {начало} до {конец} возможны перерывы подключения на сервере {страна}. После завершения ничего переустанавливать не нужно.</pre>\n\n<pre>🔑 Требуется замена ключа\n\nВаш старый ключ на сервере {страна} больше не работает. Откройте «Мои ключи», выберите его и нажмите «Заменить нерабочий ключ».</pre>\n\n<pre>✅ Новый сервер доступен\n\nДобавлена локация {страна} на AWG 1.0. Приобрести подключение можно в разделе «Купить ключ».</pre>\n\n<pre>💳 Напоминание об оплате\n\nСрок ключа {ключ} истекает {дата}. Продлить его можно из карточки ключа.</pre>")
                .parse_mode(ParseMode::Html).reply_markup(menu::broadcast_templates_menu()).await?;
        }
        Action::BroadcastAudience(audience) => {
            let server_segment = audience
                .strip_prefix("server:")
                .and_then(|value| value.parse::<i64>().ok())
                .and_then(|id| settings.vpn_server(id));
            if matches!(audience.as_str(), "all" | "active" | "expiring" | "nokeys")
                || server_segment.is_some()
            {
                let label = server_segment
                    .as_ref()
                    .map(|server| format!("владельцы ключей сервера «{}»", server.name))
                    .unwrap_or_else(|| audience.clone());
                bot.send_message(chat,format!("Сегмент: {label}.\n\nОтправьте сообщение для предпросмотра; поддерживаются текст, фото и документы.\n\nРекомендуемый текст:\n❌ Ваш старый VPN-ключ больше не работает. Откройте «🔑 Мои ключи», выберите подключение со статусом «требуется замена» и нажмите «🛟 Заменить нерабочий ключ». Новый ключ будет выдан автоматически.")).await?;
                dialogue
                    .update(State::AwaitingBroadcast { audience })
                    .await?;
            }
        }
        Action::BroadcastRetry(id) => {
            let Some(run) = settings.broadcast_run(id) else {
                bot.send_message(chat, "Отчёт рассылки не найден.")
                    .reply_markup(menu::admin_communication_hub())
                    .await?;
                return Ok(());
            };
            let recipients = settings.failed_broadcast_recipients(id);
            let mut recovered = 0usize;
            let mut failed = 0usize;
            for user_id in recipients {
                match bot
                    .copy_message(
                        ChatId(user_id),
                        ChatId(run.source_chat_id),
                        MessageId(run.source_message_id),
                    )
                    .await
                {
                    Ok(_) => {
                        recovered += 1;
                        settings.record_broadcast_delivery(id, user_id, true, None, now_epoch());
                    }
                    Err(error) => {
                        failed += 1;
                        settings.record_broadcast_delivery(
                            id,
                            user_id,
                            false,
                            Some(&error.to_string()),
                            now_epoch(),
                        );
                    }
                }
                tokio::time::sleep(std::time::Duration::from_millis(40)).await;
            }
            bot.send_message(chat,format!("🔁 Повтор рассылки #{id} завершён.\n\nДоставлено при повторе: {recovered}\nОсталось ошибок: {failed}"))
                .reply_markup(menu::broadcast_report_menu(id,failed>0)).await?;
        }
        Action::AdminHelp => {
            bot.send_message(chat,"ℹ️ Навигация\n\n➕ Создать ключ — один новый клиент\n📦 Создать оптом — пакет последовательных ключей\n📊 Статистика — трафик, активность, лидеры и подразделы\n🔎 Поиск — ключи и владельцы\n🧰 Массовое управление — включение, отключение и продление по префиксу\n🧑‍💼 Роли — назначение сотрудников\n\nВсе основные действия выполняются кнопками; текстовые команды оставлены только для совместимости.").reply_markup(menu::admin_dashboard_menu()).await?;
        }
        Action::AdminPromos => {
            bot.send_message(chat,"🎟 Управление промокодами\n\nСкидочный код уменьшает цену одной следующей покупки. Legacy-код подтверждает право пользователя подавать неограниченное количество заявок на ранее купленные лично у администратора ключи до 01.12.2026.").reply_markup(menu::admin_promos_menu()).await?;
        }
        Action::AdminCommerce => {
            let rub = [1, 3, 6, 12]
                .map(|months| settings.tariff_price_kopecks(months).unwrap_or(0) / 100);
            let stars = [1, 3, 6, 12].map(|months| settings.tariff_price_stars(months));
            let stars_text = if stars.iter().all(Option::is_some) {
                format!(
                    "{} / {} / {} / {} ⭐",
                    stars[0].unwrap_or(0),
                    stars[1].unwrap_or(0),
                    stars[2].unwrap_or(0),
                    stars[3].unwrap_or(0)
                )
            } else {
                "не настроены — оплата выключена".to_string()
            };
            bot.send_message(chat, format!("🏷 Цены и промокоды\n\nТарифы 1 / 3 / 6 / 12 мес.:\n₽ {} / {} / {} / {}\n⭐ {stars_text}\n\nРеферальное вознаграждение: {}%\nLegacy-продление: {:.2} ₽", rub[0], rub[1], rub[2], rub[3], settings.referral_percent(), settings.legacy_renewal_price_kopecks() as f64 / 100.0))
                .reply_markup(menu::admin_commerce_menu()).await?;
        }
        Action::AdminPricesRub => {
            bot.send_message(chat, "Введите четыре цены в рублях для 1 / 3 / 6 / 12 месяцев через пробел.\nНапример: 200 600 1000 2000").await?;
            dialogue.update(State::AwaitingTariffPricesRub).await?;
        }
        Action::AdminPricesStars => {
            bot.send_message(chat, "Введите четыре цены в Telegram Stars для 1 / 3 / 6 / 12 месяцев через пробел.\nНапример: 100 250 450 800\n\nДо сохранения оплата Stars недоступна.").await?;
            dialogue.update(State::AwaitingTariffPricesStars).await?;
        }
        Action::AdminReferral => {
            bot.send_message(
                chat,
                format!(
                    "Текущее реферальное вознаграждение: {}%.\nВведите новое значение от 0 до 100:",
                    settings.referral_percent()
                ),
            )
            .await?;
            dialogue.update(State::AwaitingReferralPercent).await?;
        }
        Action::Guide(kind) => {
            let text = match kind.as_str() {
                "amnezia" => "📱 AmneziaVPN\n\n1. Установите и откройте AmneziaVPN.\n2. Нажмите «+» и выберите импорт подключения.\n3. Импортируйте присланный ботом файл .conf или VPN-ссылку.\n4. Сохраните подключение и включите VPN.\n\nНе добавляйте один ключ одновременно на несколько устройств.",
                "awg" => "🛡 AmneziaWG\n\n1. Откройте AmneziaWG и нажмите «+».\n2. Выберите импорт туннеля из файла.\n3. Укажите присланный ботом файл .conf.\n4. Разрешите создание VPN-подключения и включите туннель.\n\nДля второго устройства приобретите отдельный ключ.",
                "trouble" => "🩺 Если VPN не подключается\n\n1. Выключите и снова включите туннель.\n2. Переключитесь между Wi‑Fi и мобильной сетью.\n3. Проверьте, что на другом устройстве этот ключ выключен.\n4. Откройте «Мои ключи» → нужный ключ → «Обновить подключение».\n5. Если проблема осталась, создайте обращение в поддержку.",
                _ => "Инструкция не найдена.",
            };
            bot.send_message(chat, text)
                .reply_markup(menu::instructions_menu())
                .await?;
        }
        Action::LegacyRestore => {
            legacy_admin_screen(&bot, chat, &settings).await?;
        }
        Action::LegacyPriceAsk => {
            bot.send_message(chat,format!("Текущая цена ежегодного legacy-продления: {:.2} ₽\nВведите новую цену в рублях:",settings.legacy_renewal_price_kopecks() as f64/100.0)).await?;
            dialogue.update(State::AwaitingLegacyPrice).await?;
        }
        Action::PromoInput => {
            bot.send_message(chat, "Введите технический промокод:")
                .await?;
            dialogue.update(State::AwaitingCustomerPromo).await?;
        }
        Action::LegacyRequestNew => {
            if crate::calendar::legacy_requests_open(now_epoch())
                && settings.legacy_user_eligible(uid, now_epoch())
            {
                bot.send_message(chat,"Введите желаемое имя нового ключа. После пробела можно добавить комментарий.\nПример: phone Второй ранее купленный ключ").await?;
                dialogue.update(State::AwaitingLegacyRequest).await?;
            } else {
                bot.send_message(
                    chat,
                    "Сначала активируйте Legacy-промокод. Приём заявок закрывается 01.12.2026.",
                )
                .reply_markup(menu::legacy_restore_menu(false))
                .await?;
            }
        }
        Action::LegacyRequestApprove(id) => {
            let Some(request) = settings
                .legacy_request(id)
                .filter(|r| r.status == "pending")
            else {
                return Ok(());
            };
            if !settings.claim_legacy_request(id, uid, now_epoch()) {
                bot.send_message(chat, "Эта заявка уже обрабатывается или была завершена.")
                    .await?;
                return Ok(());
            }
            let fallback = format!("legacy_{}", request.user_id);
            let base = crate::vpn::validate::normalize_name(&request.requested_name, None)
                .unwrap_or(fallback);
            let existing = settings
                .active_client_names()
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let name = crate::vpn::validate::gen_available_names(&base, 1, &existing)
                .map_err(|e| {
                    settings.release_legacy_request_claim(id);
                    crate::error::Error::Parse(e.to_string())
                })?
                .remove(0);
            let Some(server) = legacy_recovery_server(&settings) else {
                settings.release_legacy_request_claim(id);
                bot.send_message(chat, "Сервер восстановления не настроен или недоступен. Откройте карточку рабочего AWG-сервера и нажмите «🎯 Новые ключи и замена».")
                    .reply_markup(menu::admin_dashboard_menu())
                    .await?;
                return Ok(());
            };
            let created = if server.is_local {
                vpn.add(&name, None, settings.psk_default()).await
            } else {
                match nonlocal_add(&vpn, &settings, &server, &name).await {
                    Ok(result) => Ok(result),
                    Err(create_error) if server.protocol == "amneziawg-panel" => {
                        match nonlocal_files(&vpn, &settings, &server, &name).await {
                            Ok(result) => {
                                tracing::warn!(client = %name, %create_error, "восстановление подхватило ранее созданный ключ панели");
                                Ok(result)
                            }
                            Err(_) => Err(create_error),
                        }
                    }
                    Err(error) => Err(error),
                }
            };
            match created {
                Ok(result) => {
                    let expiry_result = if server.is_local {
                        vpn.set_client_expiry(&name, Some(crate::calendar::LEGACY_RESTORE_DEADLINE))
                            .await
                    } else {
                        nonlocal_set_expiry(
                            &vpn,
                            &settings,
                            &server,
                            &name,
                            crate::calendar::LEGACY_RESTORE_DEADLINE,
                        )
                        .await
                    };
                    if let Err(error) = expiry_result {
                        if server.is_local {
                            let _ = vpn.remove(&name).await;
                        } else {
                            let _ = nonlocal_remove(&vpn, &settings, &server, &name).await;
                        }
                        settings.release_legacy_request_claim(id);
                        tracing::error!(request_id = id, client = %name, %error, "срок восстановленного ключа не установлен");
                        bot.send_message(
                            chat,
                            format!(
                                "{}\n\nСервер восстановления: {} · {} · {}\nДиагностика для администратора: {error}",
                                i18n::error_text(lang, &error),
                                server.name,
                                server.location,
                                if server.is_local { "локальный" } else { &server.protocol }
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                    settings.assign_client_group(&name, None, now_epoch());
                    settings.assign_client_owner(&name, Some(request.user_id));
                    if !settings.assign_client_server(&name, server.id, &server.protocol) {
                        if server.is_local {
                            let _ = vpn.remove(&name).await;
                        } else {
                            let _ = nonlocal_remove(&vpn, &settings, &server, &name).await;
                        }
                        settings.release_legacy_request_claim(id);
                        bot.send_message(chat, "Не удалось закрепить восстановленный ключ за сервером. Созданный ключ удалён.").await?;
                        return Ok(());
                    }
                    settings.mark_legacy_subscription(&name, request.user_id, now_epoch());
                    if settings.decide_legacy_request(id, uid, Some(&name), None, now_epoch()) {
                        let user = settings.user(request.user_id);
                        let username = user
                            .and_then(|u| u.username.map(|v| format!("@{v}")))
                            .unwrap_or_else(|| "без username".into());
                        bot.send_message(ChatId(request.user_id),format!("✅ Заявка #{id} одобрена. Новый технический ключ «{name}» создан бесплатно до 31.12.2026.")).await?;
                        render::send_client_files(
                            &bot,
                            ChatId(request.user_id),
                            settings.lang(request.user_id),
                            &result,
                        )
                        .await?;
                        bot.send_message(
                            chat,
                            format!(
                                "✅ Создан ключ «{name}» для {username}, Telegram ID {}.",
                                request.user_id
                            ),
                        )
                        .reply_markup(menu::admin_dashboard_menu())
                        .await?;
                    }
                }
                Err(error) => {
                    settings.release_legacy_request_claim(id);
                    tracing::error!(request_id = id, client = %name, %error, "восстановление legacy-ключа не выполнено");
                    bot.send_message(
                        chat,
                        format!(
                            "{}\n\nСервер восстановления: {} · {} · {}\nДиагностика для администратора: {error}",
                            i18n::error_text(lang, &error),
                            server.name,
                            server.location,
                            if server.is_local { "локальный" } else { &server.protocol }
                        ),
                    )
                    .await?;
                }
            }
        }
        Action::LegacyRequestReject(id) => {
            bot.send_message(chat, format!("Введите причину отказа по заявке #{id}:"))
                .await?;
            dialogue.update(State::AwaitingLegacyReject { id }).await?;
        }
        Action::AdminPromoAction(kind) => {
            if matches!(kind.as_str(), "discount" | "legacy") {
                let prompt = if kind == "legacy" {
                    "Введите CODE [MAX_USES].\nПример: RESTORE2026 50"
                } else {
                    "Введите CODE PERCENT [MAX_USES].\nПример: FRIEND25 25 100"
                };
                bot.send_message(chat, prompt).await?;
                dialogue.update(State::AwaitingPromoCode { kind }).await?;
            }
        }
        Action::AdminSearch => {
            bot.send_message(
                chat,
                "Введите имя ключа, устройство, Telegram ID или username владельца:",
            )
            .await?;
            dialogue.update(State::AwaitingAdminSearch).await?;
        }
        Action::AdminRoles => {
            bot.send_message(chat, "🧑‍💼 Управление ролями\nВыберите действие:")
                .reply_markup(menu::admin_roles_menu())
                .await?;
        }
        Action::AdminRoleAction(operation) => {
            if matches!(operation.as_str(), "add" | "remove") {
                let prompt = if operation == "add" {
                    "Введите Telegram ID и роль: technical, support или finance.\nПример: 123456789 support"
                } else {
                    "Введите Telegram ID сотрудника, у которого нужно убрать роль."
                };
                bot.send_message(chat, prompt).await?;
                dialogue
                    .update(State::AwaitingStaffRole { operation })
                    .await?;
            }
        }
        Action::AdminBulk(operation) => {
            if operation == "menu" {
                bot.send_message(chat, "🧰 Массовое управление ключами\nВыберите операцию:")
                    .reply_markup(menu::bulk_manage_menu())
                    .await?;
            } else if matches!(operation.as_str(), "disable" | "enable" | "extend") {
                let prompt = if operation == "extend" {
                    "Введите префикс и срок, например: client 30d"
                } else {
                    "Введите префикс имён ключей, например: client"
                };
                bot.send_message(chat, prompt).await?;
                dialogue
                    .update(State::AwaitingBulkManage { operation })
                    .await?;
            }
        }
        Action::AdminBulkConfirm => {
            let State::AwaitingBulkConfirm {
                operation,
                prefix,
                names,
                seconds,
            } = dialogue.get().await?.unwrap_or_default()
            else {
                bot.send_message(chat, "Операция уже завершена или отменена.")
                    .reply_markup(menu::admin_dashboard_menu())
                    .await?;
                return Ok(());
            };
            let mut ok = 0usize;
            for name in &names {
                let result = match operation.as_str() {
                    "disable" => vpn.disable_client(name).await,
                    "enable" => vpn.enable_client(name).await,
                    "extend" => vpn
                        .extend_client(name, seconds.unwrap_or(0), now_epoch())
                        .await
                        .map(|_| ()),
                    _ => Err(crate::error::Error::Parse("неизвестная операция".into())),
                };
                if result.is_ok() {
                    ok += 1;
                    settings.log_event(
                        now_epoch(),
                        EventKind::Modify,
                        Some(name),
                        Some(uid),
                        Some(&format!("bulk_{operation}")),
                    );
                }
            }
            bot.send_message(
                chat,
                format!(
                    "✅ Обработано {ok}/{} ключей с префиксом {prefix}.",
                    names.len()
                ),
            )
            .reply_markup(menu::admin_dashboard_menu())
            .await?;
            dialogue.update(State::Idle).await?;
        }
        Action::AdminUser(user_id) => admin_user_screen(&bot, chat, &settings, user_id).await?,
        Action::AdminUserKeys(user_id) => {
            let names = settings.user_client_names(user_id);
            let text = if names.is_empty() {
                "Ключей нет.".into()
            } else {
                names
                    .iter()
                    .map(|n| {
                        let status = if vpn.client_disabled(n) { "⏸" } else { "✅" };
                        let expiry = crate::vpn::model::format_expiry(
                            settings.lang(uid),
                            now_epoch(),
                            vpn.client_expiry(n),
                        );
                        format!("{status} {n} · {expiry}")
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            bot.send_message(chat, format!("🔑 Ключи пользователя {user_id}\n\n{text}"))
                .reply_markup(menu::admin_user_keys_menu(user_id, &names))
                .await?;
        }
        Action::AdminUserDeleteKeysAsk(user_id) => {
            let names = settings.user_client_names(user_id);
            if names.is_empty() {
                bot.send_message(chat, "У пользователя нет активных ключей.")
                    .reply_markup(menu::admin_user_menu(
                        user_id,
                        settings.user_blocked(user_id),
                    ))
                    .await?;
            } else {
                bot.send_message(
                    chat,
                    format!(
                        "⚠️ Удаление всех ключей пользователя {user_id}\n\nБудут удалены {} ключей с соответствующих AWG-серверов и панелей:\n\n{}\n\nПлатежи, баланс и карточка пользователя сохранятся. Операцию нельзя отменить.",
                        names.len(),
                        names.join("\n")
                    ),
                )
                .reply_markup(menu::admin_user_delete_keys_confirm_menu(user_id))
                .await?;
            }
        }
        Action::AdminUserDeleteKeysConfirm(user_id) => {
            let names = settings.user_client_names(user_id);
            let mut removed = Vec::new();
            let mut failed = Vec::new();
            for name in names {
                match client_remove(&vpn, &settings, &name).await {
                    Ok(()) => {
                        settings.log_event(
                            now_epoch(),
                            EventKind::ClientRemove,
                            Some(&name),
                            Some(uid),
                            Some(&format!("all user keys removed; user={user_id}")),
                        );
                        removed.push(name);
                    }
                    Err(error) => failed.push(format!("{name}: {error}")),
                }
            }
            let mut text = format!(
                "🗑 Удаление ключей пользователя {user_id}\n\nУдалено: {}",
                removed.len()
            );
            if !failed.is_empty() {
                text.push_str(&format!(
                    "\nНе удалось удалить: {}\n\n{}",
                    failed.len(),
                    failed.join("\n")
                ));
            }
            bot.send_message(chat, text)
                .reply_markup(menu::admin_user_menu(
                    user_id,
                    settings.user_blocked(user_id),
                ))
                .await?;
        }
        Action::AdminUserPayments(user_id) => {
            let rows = settings.user_payments(user_id, 20);
            let text = if rows.is_empty() {
                "Платежей нет.".into()
            } else {
                rows.iter()
                    .map(|p| {
                        format!(
                            "#{} · {:?} · {:.2} ₽ · {} мес. · {}",
                            p.id,
                            p.status,
                            p.amount_kopecks as f64 / 100.0,
                            p.months,
                            p.client_name.as_deref().unwrap_or("без ключа")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            bot.send_message(chat, format!("💳 Платежи пользователя {user_id}\n\n{text}"))
                .reply_markup(menu::admin_user_menu(
                    user_id,
                    settings.user_blocked(user_id),
                ))
                .await?;
        }
        Action::AdminUserBalance(user_id) => {
            bot.send_message(
                chat,
                "Введите сумму и причину.\nПополнение: 500 Бонус за перенос\nСписание: -200 Возврат ошибочного начисления",
            )
            .await?;
            dialogue
                .update(State::AwaitingUserBalance { user_id })
                .await?;
        }
        Action::AdminUserDiscount(user_id) => {
            bot.send_message(chat, "Введите скидку в процентах: 25 — бессрочно; 25 2027-12-31 — до даты; clear — удалить.")
                .await?;
            dialogue
                .update(State::AwaitingUserDiscount { user_id })
                .await?;
        }
        Action::AdminUserNote(user_id) => {
            bot.send_message(
                chat,
                "Введите внутреннюю заметку (до 500 символов). Для удаления отправьте clear.",
            )
            .await?;
            dialogue.update(State::AwaitingUserNote { user_id }).await?;
        }
        Action::AdminUserBlock(user_id, blocked) => {
            let changed = settings.set_user_blocked(user_id, blocked);
            if changed {
                settings.log_event(
                    now_epoch(),
                    EventKind::Modify,
                    None,
                    Some(uid),
                    Some(&format!("user={user_id} blocked={blocked}")),
                );
            }
            admin_user_screen(&bot, chat, &settings, user_id).await?;
        }
        Action::ClientNoteAsk(name) => {
            bot.send_message(
                chat,
                format!(
                    "Введите внутреннюю заметку для ключа {name}. Для удаления отправьте clear."
                ),
            )
            .await?;
            dialogue.update(State::AwaitingClientNote { name }).await?;
        }
        Action::StatsSection(section) => {
            let now = now_epoch();
            let text = match section.as_str() {
                "servers" => {
                    let mut lines = Vec::new();
                    for server in settings.vpn_servers() {
                        let assigned = settings.server_client_count(server.id);
                        if server.protocol == "amneziawg-panel" {
                            let panel = match settings.panel_password(server.id) {
                                Some(secret) => vpn.panel_clients(&server, &secret).await,
                                None => Err(crate::error::Error::Parse(
                                    "пароль панели не настроен".into(),
                                )),
                            };
                            match panel {
                                Ok(clients) => {
                                    settings.ingest_panel(
                                        server.id,
                                        now,
                                        &clients
                                            .iter()
                                            .map(|client| crate::store::Sample {
                                                name: client.name.clone(),
                                                ip: client.address.clone(),
                                                rx: client.transfer_rx,
                                                tx: client.transfer_tx,
                                                last_handshake: client.last_handshake_epoch(),
                                            })
                                            .collect::<Vec<_>>(),
                                    );
                                    let online = clients
                                        .iter()
                                        .filter(|client| {
                                            client.last_handshake_epoch().is_some_and(|handshake| {
                                                now.saturating_sub(handshake)
                                                    < crate::vpn::model::ONLINE_THRESHOLD_SECS
                                            })
                                        })
                                        .count();
                                    let traffic = clients.iter().fold(0_u64, |total, client| {
                                        total
                                            .saturating_add(client.transfer_rx)
                                            .saturating_add(client.transfer_tx)
                                    });
                                    lines.push(format!(
                                        "🟢 {} · {}\n   AWG 1.0 · панель доступна\n   Ключи: {} (в базе: {assigned}) · онлайн: {online}\n   Трафик панели: {}",
                                        server.name,
                                        server.location,
                                        clients.len(),
                                        crate::vpn::model::human_bytes(traffic)
                                    ));
                                }
                                Err(error) => lines.push(format!(
                                    "🔴 {} · {}\n   AWG 1.0 · панель недоступна\n   Ключи в базе: {assigned}\n   Причина: {error}",
                                    server.name, server.location
                                )),
                            }
                        } else {
                            lines.push(format!(
                                "{} {} · {}\n   {} · ключей: {assigned}/{}, статус: {}",
                                if server.status == "online" {
                                    "🟢"
                                } else {
                                    "⚪"
                                },
                                server.name,
                                server.location,
                                server.protocol,
                                server.capacity,
                                server.status
                            ));
                        }
                    }
                    format!(
                        "🖥 Статистика серверов\n\n{}",
                        if lines.is_empty() {
                            "Серверы не добавлены.".into()
                        } else {
                            lines.join("\n\n")
                        }
                    )
                }
                "users" => {
                    let s = settings.admin_user_stats(now);
                    format!("👤 Пользователи\n\nВсего: {}\nНовых сегодня: {}\nНовых за 30 дней: {}\nПлатящих: {}\nПришли по рефералам: {}\nЗаблокировано: {}",s.total,s.new_today,s.new_30d,s.paying,s.referred,s.blocked)
                }
                "subscriptions" => {
                    let clients = vpn.list().await.unwrap_or_default();
                    let active = clients
                        .iter()
                        .filter(|c| {
                            !vpn.client_disabled(&c.name)
                                && vpn.client_expiry(&c.name).is_none_or(|e| e > now)
                        })
                        .count();
                    let expiring = clients
                        .iter()
                        .filter(|c| {
                            vpn.client_expiry(&c.name)
                                .is_some_and(|e| e > now && e - now <= 7 * 86_400)
                        })
                        .count();
                    format!("💳 Подписки\n\nАктивных: {active}\nИстекают за 7 дней: {expiring}\nОтключено: {}\nВсего ключей: {}",clients.iter().filter(|c|vpn.client_disabled(&c.name)).count(),clients.len())
                }
                "tariffs" => {
                    let rows = settings.recent_payments(100_000);
                    let approved = rows
                        .iter()
                        .filter(|p| {
                            p.status == crate::store::PaymentStatus::Approved && p.months > 0
                        })
                        .collect::<Vec<_>>();
                    format!("📈 Популярность тарифов\n\n1 месяц: {}\n3 месяца: {}\n6 месяцев: {}\n12 месяцев: {}",approved.iter().filter(|p|p.months==1).count(),approved.iter().filter(|p|p.months==3).count(),approved.iter().filter(|p|p.months==6).count(),approved.iter().filter(|p|p.months==12).count())
                }
                _ => "Раздел статистики не найден.".into(),
            };
            bot.send_message(chat, text)
                .reply_markup(menu::statistics_menu())
                .await?;
        }
        Action::Buy => {
            let servers = settings.available_vpn_servers();
            bot.send_message(chat, if servers.is_empty() { "Сейчас нет доступных локаций. Администратор уже может проверить лимиты серверов." } else { "Выберите локацию и протокол:" })
                .reply_markup(menu::buy_servers_menu(&servers, &settings))
                .await?;
        }
        Action::BuyServer(server_id) => {
            let available = settings
                .available_vpn_servers()
                .into_iter()
                .any(|server| server.id == server_id);
            if available && settings.set_purchase_server(uid, server_id, now_epoch()) {
                bot.send_message(chat, "📅 Шаг 2 из 3 · Выберите срок подписки:")
                    .reply_markup(menu::buy_terms_menu([
                        settings.tariff_price_kopecks(1).unwrap_or(0),
                        settings.tariff_price_kopecks(3).unwrap_or(0),
                        settings.tariff_price_kopecks(6).unwrap_or(0),
                        settings.tariff_price_kopecks(12).unwrap_or(0),
                    ]))
                    .await?;
            } else {
                bot.send_message(chat, "Эта локация заполнена или временно недоступна.")
                    .await?;
            }
        }
        Action::BuyTerm(months) => {
            if tariff_duration(months).is_some() {
                if settings.purchase_server(uid).is_none() {
                    let servers = settings.available_vpn_servers();
                    bot.send_message(chat, "Сначала выберите сервер подключения:")
                        .reply_markup(menu::buy_servers_menu(&servers, &settings))
                        .await?;
                    return Ok(());
                }
                bot.send_message(chat, "💳 Шаг 3 из 3 · Выберите способ оплаты:")
                    .reply_markup(menu::buy_method_menu(
                        months,
                        settings.acquiring_url_template().is_some(),
                    ))
                    .await?;
            }
        }
        Action::BuyMethod(months, method) => {
            let Some(base_amount) = settings.tariff_price_kopecks(months) else {
                return Ok(());
            };
            if method == "stars" {
                let Some(stars) = settings
                    .tariff_price_stars(months)
                    .filter(|value| *value > 0)
                else {
                    bot.send_message(
                        chat,
                        "Оплата Telegram Stars пока не настроена администратором.",
                    )
                    .await?;
                    return Ok(());
                };
                let Some(server_id) = settings.purchase_server(uid) else {
                    bot.send_message(chat, "Сначала выберите локацию.").await?;
                    return Ok(());
                };
                if let Some(order) = settings.create_star_order(crate::store::NewStarOrder {
                    user_id: uid,
                    kind: "purchase",
                    months,
                    stars,
                    client_name: None,
                    server_id: Some(server_id),
                    created_at: now_epoch(),
                }) {
                    send_star_invoice(&bot, chat, &order).await?;
                }
                return Ok(());
            }
            let discount = settings.purchase_discount(uid, now_epoch());
            let amount = base_amount.saturating_mul(100 - discount.clamp(0, 100)) / 100;
            if method == "acquiring" {
                let Some(server_id) = settings.purchase_server(uid) else {
                    return Ok(());
                };
                let Some(template) = settings.acquiring_url_template() else {
                    bot.send_message(chat, "Онлайн-оплата пока не настроена.")
                        .await?;
                    return Ok(());
                };
                if let Some(id) = settings.create_payment_request_on_server(
                    uid,
                    months,
                    amount,
                    "acquiring",
                    server_id,
                    now_epoch(),
                ) {
                    let url = template
                        .replace("{order_id}", &id.to_string())
                        .replace("{amount}", &amount.to_string())
                        .replace("{user_id}", &uid.to_string());
                    bot.send_message(chat, format!("🏦 Онлайн-оплата · заказ #{id}\n\nК оплате: {:.2} ₽\n\nПерейдите по защищённой ссылке платёжного провайдера:\n{url}\n\nПосле подключения webhook подтверждение и выдача ключа будут выполняться автоматически; пока заказ подтверждается администратором.", amount as f64/100.0)).await?;
                }
            } else if method == "balance" {
                let Some(server_id) = settings.purchase_server(uid).filter(|selected| {
                    settings
                        .available_vpn_servers()
                        .iter()
                        .any(|server| server.id == *selected)
                }) else {
                    bot.send_message(
                        chat,
                        "Выбранный сервер больше недоступен. Выберите рабочую локацию заново.",
                    )
                    .reply_markup(menu::buy_servers_menu(
                        &settings.available_vpn_servers(),
                        &settings,
                    ))
                    .await?;
                    return Ok(());
                };
                let nonce: u64 = rand::random();
                let reference = format!("balance:{uid}:{server_id}:{}:{nonce}", now_epoch());
                if !settings.spend_balance(uid, amount, &reference, now_epoch()) {
                    bot.send_message(chat, format!("Недостаточно средств на внутреннем балансе. Доступно: {:.2} ₽, требуется: {:.2} ₽.", settings.balance_kopecks(uid) as f64 / 100.0, amount as f64 / 100.0))
                        .reply_markup(menu::customer_keyboard())
                        .await?;
                    return Ok(());
                }
                match provision_customer_key(&vpn, &settings, uid, months, server_id).await {
                    Ok(result) => {
                        if let Some(referrer) = settings.user(uid).and_then(|u| u.referrer_id) {
                            let reward = amount * i64::from(settings.referral_percent()) / 100;
                            let _ = settings.add_ledger_entry(
                                referrer,
                                reward,
                                "referral",
                                &format!("referral:{reference}"),
                                Some(&format!("user={uid}")),
                                now_epoch(),
                            );
                            let _ = bot
                                .send_message(
                                    ChatId(referrer),
                                    format!(
                                        "🎁 Реферальное начисление: {:.2} ₽",
                                        reward as f64 / 100.0
                                    ),
                                )
                                .await;
                        }
                        settings.log_event(
                            now_epoch(),
                            EventKind::ClientAdd,
                            Some(&result.name),
                            Some(uid),
                            Some("balance_purchase"),
                        );
                        bot.send_message(chat, format!("✅ Ключ {} создан.", result.name))
                            .await?;
                        if let Err(e) = render::send_client_files(&bot, chat, lang, &result).await {
                            tracing::error!(error = %e, "не удалось выдать купленный ключ");
                        }
                    }
                    Err(e) => {
                        let _ = settings.add_ledger_entry(
                            uid,
                            amount,
                            "refund",
                            &format!("refund:{reference}"),
                            Some("provision failed"),
                            now_epoch(),
                        );
                        bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                    }
                }
            } else if method == "manual" {
                let server_id = settings.purchase_server(uid).or_else(|| {
                    settings.default_vpn_server().filter(|id| {
                        settings
                            .available_vpn_servers()
                            .iter()
                            .any(|server| server.id == *id)
                    })
                });
                if let Some(id) = server_id.and_then(|server_id| {
                    settings.create_payment_request_on_server(
                        uid,
                        months,
                        amount,
                        "manual",
                        server_id,
                        now_epoch(),
                    )
                }) {
                    let location = settings
                        .purchase_server(uid)
                        .and_then(|server_id| settings.vpn_server(server_id))
                        .map(|server| server.location)
                        .unwrap_or_else(|| "основной сервер".into());
                    let text = format!(
                        "💳 Оплата переводом · заявка #{id}\n\n📍 Сервер: {location}\n📅 Тариф: {months} мес.\n💰 К оплате: {:.2} ₽\n\n{}\n\nПосле перевода нажмите «✅ Я оплатил» и пришлите чек, номер операции или комментарий.",
                        amount as f64 / 100.0,
                        settings.payment_instructions()
                    );
                    bot.send_message(chat, text)
                        .reply_markup(menu::payment_paid_menu(id))
                        .await?;
                } else {
                    let servers = settings.available_vpn_servers();
                    bot.send_message(
                        chat,
                        "Не удалось создать заявку: сначала выберите рабочий сервер подключения.",
                    )
                    .reply_markup(menu::buy_servers_menu(&servers, &settings))
                    .await?;
                }
            }
        }
        Action::BuyPaid(id) => {
            if settings.payment_request(id).is_some_and(|p| {
                p.user_id == uid && p.status == crate::store::PaymentStatus::Pending
            }) {
                bot.send_message(
                    chat,
                    "Пришлите номер операции, комментарий или скриншот чека.",
                )
                .await?;
                dialogue.update(State::AwaitingPaymentProof { id }).await?;
            }
        }
        Action::MyKeys => {
            let (lines, buttons) = customer_key_list(&settings, &vpn, uid);
            let text = if lines.is_empty() {
                "У вас пока нет ключей.".to_string()
            } else {
                format!("🔑 Ваши подключения\n\n{}\n\n❌ Нерабочий ключ откройте и замените кнопкой восстановления.", lines.join("\n\n"))
            };
            let mut request = bot.send_message(chat, text);
            if !buttons.is_empty() {
                request = request.reply_markup(menu::customer_keys_menu(&buttons));
            }
            request.await?;
        }
        Action::Balance => {
            bot.send_message(
                chat,
                format!(
                    "💰 Баланс: {:.2} ₽",
                    settings.balance_kopecks(uid) as f64 / 100.0
                ),
            )
            .reply_markup(menu::customer_keyboard())
            .await?;
        }
        Action::Renew(name) => {
            if settings.client_owner(&name) == Some(uid) {
                if settings.is_legacy_client(&name, uid) {
                    let price = settings.legacy_renewal_price_for_user(
                        uid,
                        settings.legacy_renewal_price_kopecks(),
                    );
                    let target = crate::calendar::legacy_renewal_target(
                        now_epoch(),
                        vpn.client_expiry(&name),
                    );
                    bot.send_message(chat,format!("🔧 Технический тариф\n\nКлюч: {name}\nПродление: {:.2} ₽\nНовый срок: 31.12.{}",price as f64 / 100.0,crate::calendar::year_at(target))).reply_markup(menu::legacy_renew_menu(&name, price)).await?;
                    return Ok(());
                }
                bot.send_message(chat, format!("Выберите срок продления ключа {name}:"))
                    .reply_markup(menu::renew_terms_menu(
                        &name,
                        [
                            settings.tariff_price_kopecks(1).unwrap_or(0),
                            settings.tariff_price_kopecks(3).unwrap_or(0),
                            settings.tariff_price_kopecks(6).unwrap_or(0),
                            settings.tariff_price_kopecks(12).unwrap_or(0),
                        ],
                    ))
                    .await?;
                let auto = settings
                    .auto_renew(&name, uid)
                    .map(|(m, on)| {
                        if on {
                            format!("Сейчас включено: {m} мес.")
                        } else {
                            "Сейчас выключено".to_string()
                        }
                    })
                    .unwrap_or_else(|| "Сейчас выключено".to_string());
                bot.send_message(chat, format!("🔁 Автопродление\n{auto}"))
                    .reply_markup(menu::auto_renew_menu(&name))
                    .await?;
            } else {
                bot.send_message(chat, "Ключ уже удалён после истечения. Обратитесь в поддержку для восстановления или приобретите новый ключ.").await?;
            }
        }
        Action::LegacyRenew(name) => {
            if settings.client_owner(&name) == Some(uid) && settings.is_legacy_client(&name, uid) {
                let target =
                    crate::calendar::legacy_renewal_target(now_epoch(), vpn.client_expiry(&name));
                let price = settings
                    .legacy_renewal_price_for_user(uid, settings.legacy_renewal_price_kopecks());
                bot.send_message(chat,format!("Продление ключа «{name}» до 31.12.{} стоит {:.2} ₽. Выберите способ оплаты:",crate::calendar::year_at(target),price as f64 / 100.0)).reply_markup(menu::legacy_renew_method_menu(&name)).await?;
            }
        }
        Action::LegacyRenewMethod(name, method) => {
            if settings.client_owner(&name) != Some(uid) || !settings.is_legacy_client(&name, uid) {
                return Ok(());
            }
            let amount = settings
                .legacy_renewal_price_for_user(uid, settings.legacy_renewal_price_kopecks());
            let target =
                crate::calendar::legacy_renewal_target(now_epoch(), vpn.client_expiry(&name));
            if method == "balance" {
                let reference = format!("legacy-renew:{uid}:{name}:{target}");
                if !settings.spend_balance(uid, amount, &reference, now_epoch()) {
                    bot.send_message(chat, "Недостаточно средств на внутреннем балансе.")
                        .reply_markup(menu::customer_keyboard())
                        .await?;
                    return Ok(());
                }
                match set_managed_expiry(&vpn, &settings, &name, target).await {
                    Ok(()) => {
                        if settings
                            .client_vpn_server(&name)
                            .is_some_and(|server| server.is_local)
                            && vpn.client_disabled(&name)
                        {
                            vpn.enable_client(&name).await?;
                        }
                        bot.send_message(
                            chat,
                            format!(
                                "✅ Ключ «{name}» продлён до 31.12.{}.",
                                crate::calendar::year_at(target)
                            ),
                        )
                        .reply_markup(menu::customer_keyboard())
                        .await?;
                    }
                    Err(error) => {
                        settings.add_ledger_entry(
                            uid,
                            amount,
                            "refund",
                            &format!("refund:{reference}"),
                            Some("legacy renewal failed"),
                            now_epoch(),
                        );
                        bot.send_message(chat, i18n::error_text(lang, &error))
                            .await?;
                    }
                }
            } else if method == "manual" {
                if let Some(id) =
                    settings.create_legacy_renewal_request(uid, &name, amount, now_epoch())
                {
                    bot.send_message(chat,format!("Заявка #{id} на техническое продление ключа «{name}» до 31.12.{}\nСумма: {:.2} ₽\n\n{}",crate::calendar::year_at(target),amount as f64 / 100.0,settings.payment_instructions())).reply_markup(menu::payment_paid_menu(id)).await?;
                } else {
                    bot.send_message(
                        chat,
                        "По этому ключу уже есть заявка, ожидающая решения администратора.",
                    )
                    .reply_markup(menu::customer_keyboard())
                    .await?;
                }
            }
        }
        Action::RenewTerm(name, months) => {
            if settings.client_owner(&name) == Some(uid) && tariff_duration(months).is_some() {
                bot.send_message(chat, "Выберите способ оплаты продления:")
                    .reply_markup(menu::renew_method_menu(&name, months))
                    .await?;
            }
        }
        Action::RenewMethod(name, months, method) => {
            if settings.client_owner(&name) != Some(uid) {
                return Ok(());
            }
            let (Some(base_amount), Some(expiry)) = (
                settings.tariff_price_kopecks(months),
                tariff_duration(months),
            ) else {
                return Ok(());
            };
            if method == "stars" {
                let Some(stars) = settings
                    .tariff_price_stars(months)
                    .filter(|value| *value > 0)
                else {
                    bot.send_message(
                        chat,
                        "Оплата Telegram Stars пока не настроена администратором.",
                    )
                    .await?;
                    return Ok(());
                };
                if let Some(order) = settings.create_star_order(crate::store::NewStarOrder {
                    user_id: uid,
                    kind: "renew",
                    months,
                    stars,
                    client_name: Some(&name),
                    server_id: None,
                    created_at: now_epoch(),
                }) {
                    send_star_invoice(&bot, chat, &order).await?;
                }
                return Ok(());
            }
            let discount = settings.purchase_discount(uid, now_epoch());
            let amount = base_amount.saturating_mul(100 - discount.clamp(0, 100)) / 100;
            let seconds = duration_seconds(expiry).unwrap_or(0);
            if method == "balance" {
                let reference = format!("renew:{uid}:{name}:{}", now_epoch());
                if !settings.spend_balance(uid, amount, &reference, now_epoch()) {
                    bot.send_message(chat, "Недостаточно средств на внутреннем балансе.")
                        .await?;
                    return Ok(());
                }
                match extend_managed_client(&vpn, &settings, &name, seconds, now_epoch()).await {
                    Ok(epoch) => {
                        bot.send_message(
                            chat,
                            format!("✅ Ключ {name} продлён. Новый срок (Unix): {epoch}"),
                        )
                        .reply_markup(menu::customer_keyboard())
                        .await?;
                    }
                    Err(error) => {
                        settings.add_ledger_entry(
                            uid,
                            amount,
                            "refund",
                            &format!("refund:{reference}"),
                            Some("renew failed"),
                            now_epoch(),
                        );
                        bot.send_message(chat, i18n::error_text(lang, &error))
                            .await?;
                    }
                }
            } else if method == "manual" {
                if let Some(id) =
                    settings.create_renewal_request(uid, &name, months, amount, now_epoch())
                {
                    bot.send_message(chat, format!("Заявка #{id} на продление ключа {name}\nСрок: {months} мес.\nСумма: {} ₽\n\n{}", amount / 100, settings.payment_instructions()))
                        .reply_markup(menu::payment_paid_menu(id)).await?;
                }
            }
        }
        Action::AutoRenew(name, months, enabled) => {
            if settings.set_auto_renew(&name, uid, months, enabled, now_epoch()) {
                bot.send_message(
                    chat,
                    if enabled {
                        format!("✅ Автопродление ключа {name} включено на тариф {months} мес.")
                    } else {
                        format!("Автопродление ключа {name} выключено.")
                    },
                )
                .reply_markup(menu::customer_keyboard())
                .await?;
            }
        }
        Action::Profile => {
            let me = bot.get_me().await?;
            let username = me.username.clone().unwrap_or_default();
            bot.send_message(chat, format!("👤 Telegram ID: {uid}\nАктивных ключей: {}\nРеферальная ссылка:\nhttps://t.me/{username}?start=ref_{uid}", settings.user_client_names(uid).len()))
                .reply_markup(menu::profile_menu(cfg.portal_public_url.is_some())).await?;
        }
        Action::Portal => {
            match (
                &cfg.portal_public_url,
                settings.issue_portal_token(uid, now_epoch()),
            ) {
                (Some(base), Some(token)) => {
                    let url = format!("{base}/login?token={token}");
                    bot.send_message(chat, "🌐 Внутренний личный кабинет\n\nСсылка одноразовая и действует 15 минут. После входа сессия сохранится на этом устройстве на 30 дней. Не пересылайте ссылку другим людям.")
                        .reply_markup(menu::portal_link_menu(&url))
                        .await?;
                }
                (None, _) => {
                    bot.send_message(chat, "Веб-кабинет пока не настроен администратором.")
                        .reply_markup(menu::customer_keyboard())
                        .await?;
                }
                _ => {
                    bot.send_message(chat, "Не удалось создать ссылку входа. Повторите позже.")
                        .reply_markup(menu::customer_keyboard())
                        .await?;
                }
            }
        }
        Action::PaymentReject(id) => {
            bot.send_message(chat, format!("Укажите причину отказа по заявке #{id}:"))
                .await?;
            dialogue.update(State::AwaitingPaymentReject { id }).await?;
        }
        Action::PaymentApprove(id) => {
            let Some(req) = settings.payment_request(id) else {
                return Ok(());
            };
            if req.status != crate::store::PaymentStatus::Pending {
                return Ok(());
            }
            if req.method == "topup" {
                if settings.decide_payment(
                    id,
                    crate::store::PaymentStatus::Approved,
                    uid,
                    None,
                    now_epoch(),
                ) {
                    let _ = settings.add_ledger_entry(
                        req.user_id,
                        req.amount_kopecks,
                        "topup",
                        &format!("payment:{id}"),
                        None,
                        now_epoch(),
                    );
                    let _ = bot
                        .send_message(
                            ChatId(req.user_id),
                            format!(
                                "✅ Баланс пополнен на {:.2} ₽.",
                                req.amount_kopecks as f64 / 100.0
                            ),
                        )
                        .reply_markup(menu::customer_keyboard())
                        .await;
                    bot.send_message(chat, format!("✅ Пополнение #{id} одобрено."))
                        .await?;
                }
                return Ok(());
            }
            if let Some(name) = req.client_name.clone() {
                if settings.client_owner(&name) != Some(req.user_id) {
                    bot.send_message(
                        chat,
                        "Владелец ключа изменился — заявка не может быть одобрена.",
                    )
                    .await?;
                    return Ok(());
                }
                if req.method == "legacy_manual" {
                    if !settings.is_legacy_client(&name, req.user_id) {
                        bot.send_message(chat, "Ключ больше не относится к техническому тарифу.")
                            .await?;
                        return Ok(());
                    }
                    let target = crate::calendar::legacy_renewal_target(
                        now_epoch(),
                        vpn.client_expiry(&name),
                    );
                    match set_managed_expiry(&vpn, &settings, &name, target).await {
                        Ok(()) => {
                            if settings
                                .client_vpn_server(&name)
                                .is_some_and(|server| server.is_local)
                                && vpn.client_disabled(&name)
                            {
                                vpn.enable_client(&name).await?;
                            }
                            if settings.decide_payment(
                                id,
                                crate::store::PaymentStatus::Approved,
                                uid,
                                Some(&name),
                                now_epoch(),
                            ) {
                                let year = crate::calendar::year_at(target);
                                bot.send_message(ChatId(req.user_id),format!("✅ Оплата подтверждена. Технический ключ «{name}» продлён до 31.12.{year}.")).reply_markup(menu::customer_keyboard()).await?;
                                bot.send_message(
                                    chat,
                                    format!("✅ Техническое продление по заявке #{id} выполнено."),
                                )
                                .await?;
                            }
                        }
                        Err(error) => {
                            bot.send_message(chat, i18n::error_text(lang, &error))
                                .await?;
                        }
                    }
                    return Ok(());
                }
                let seconds = tariff_duration(req.months)
                    .and_then(duration_seconds)
                    .unwrap_or(0);
                match extend_managed_client(&vpn, &settings, &name, seconds, now_epoch()).await {
                    Ok(epoch) => {
                        if settings.decide_payment(
                            id,
                            crate::store::PaymentStatus::Approved,
                            uid,
                            Some(&name),
                            now_epoch(),
                        ) {
                            if let Some(referrer) =
                                settings.user(req.user_id).and_then(|u| u.referrer_id)
                            {
                                let reward = req.amount_kopecks
                                    * i64::from(settings.referral_percent())
                                    / 100;
                                settings.add_ledger_entry(
                                    referrer,
                                    reward,
                                    "referral",
                                    &format!("referral:payment:{id}"),
                                    Some(&format!("renew user={}", req.user_id)),
                                    now_epoch(),
                                );
                            }
                            bot.send_message(ChatId(req.user_id), format!("✅ Оплата подтверждена. Ключ {name} продлён. Новый срок (Unix): {epoch}"))
                                .reply_markup(menu::customer_keyboard()).await?;
                            bot.send_message(
                                chat,
                                format!("✅ Продление по заявке #{id} выполнено."),
                            )
                            .await?;
                        }
                    }
                    Err(error) => {
                        bot.send_message(chat, i18n::error_text(lang, &error))
                            .await?;
                    }
                }
                return Ok(());
            }
            let Some(server_id) = req
                .server_id
                .or_else(|| settings.purchase_server(req.user_id))
            else {
                bot.send_message(
                    chat,
                    "В заявке не указана локация; попросите пользователя создать новую заявку.",
                )
                .await?;
                return Ok(());
            };
            match provision_customer_key(&vpn, &settings, req.user_id, req.months, server_id).await
            {
                Ok(result) => {
                    if settings.decide_payment(
                        id,
                        crate::store::PaymentStatus::Approved,
                        uid,
                        Some(&result.name),
                        now_epoch(),
                    ) {
                        if let Some(user) = settings.user(req.user_id) {
                            if let Some(referrer) = user.referrer_id {
                                let reward = req.amount_kopecks
                                    * i64::from(settings.referral_percent())
                                    / 100;
                                let _ = settings.add_ledger_entry(
                                    referrer,
                                    reward,
                                    "referral",
                                    &format!("referral:payment:{id}"),
                                    Some(&format!("user={}", req.user_id)),
                                    now_epoch(),
                                );
                                let _ = bot
                                    .send_message(
                                        ChatId(referrer),
                                        format!(
                                            "🎁 Реферальное начисление: {:.2} ₽",
                                            reward as f64 / 100.0
                                        ),
                                    )
                                    .await;
                            }
                        }
                        settings.log_event(
                            now_epoch(),
                            EventKind::ClientAdd,
                            Some(&result.name),
                            Some(req.user_id),
                            Some("manual_purchase"),
                        );
                        let _ = bot
                            .send_message(
                                ChatId(req.user_id),
                                format!("✅ Оплата подтверждена. Ключ {} создан.", result.name),
                            )
                            .await;
                        if let Err(e) = render::send_client_files(
                            &bot,
                            ChatId(req.user_id),
                            settings.lang(req.user_id),
                            &result,
                        )
                        .await
                        {
                            tracing::error!(error = %e, "не удалось выдать оплаченный ключ");
                        }
                        bot.send_message(chat, format!("✅ Заявка #{id} одобрена."))
                            .await?;
                    }
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::AssignOwnerAsk(name) => {
            bot.send_message(
                chat,
                format!("Введите Telegram ID или @username владельца ключа {name}:"),
            )
            .await?;
            dialogue.update(State::AwaitingClientOwner { name }).await?;
        }
        Action::AdminExpiryAsk(name) => {
            if settings.client_vpn_server(&name).is_none() {
                bot.send_message(
                    chat,
                    "Ключ уже удалён или не существует. Его можно только создать заново.",
                )
                .await?;
                return Ok(());
            }
            bot.send_message(chat, format!("Текущий срок ключа {name}: {:?}\n\nВведите, на сколько продлить: 12h, 7d, 30d, 6m, 1y. Для бессрочного ключа — none. Новый период добавляется к оставшемуся сроку.", vpn.client_expiry(&name))).await?;
            dialogue.update(State::AwaitingAdminExpiry { name }).await?;
        }
        Action::SetClientEnabled(name, enabled) => {
            let result = if enabled {
                vpn.enable_client(&name).await
            } else {
                vpn.disable_client(&name).await
            };
            match result {
                Ok(()) => {
                    bot.send_message(
                        chat,
                        format!(
                            "✅ Ключ {name} {}.",
                            if enabled {
                                "включён"
                            } else {
                                "отключён"
                            }
                        ),
                    )
                    .await?
                }
                Err(error) => {
                    bot.send_message(chat, i18n::error_text(lang, &error))
                        .await?
                }
            };
        }
        Action::SupportTicket(id) => {
            if let Some(t) = settings.support_ticket(id) {
                bot.send_message(
                    chat,
                    format!(
                        "🆘 Обращение #{}\nПользователь: {}\nКатегория: {}\nПриоритет: {}\nСтатус: {}\nОтветственный: {}\n\n{}",
                        t.id,
                        t.user_id,
                        t.category,
                        t.priority,
                        t.status,
                        t.assigned_to
                            .map(|v| v.to_string())
                            .unwrap_or_else(|| "не назначен".into()),
                        t.subject
                    ),
                )
                .reply_markup(menu::support_ticket_menu(id))
                .await?;
            }
        }
        Action::SupportNewCategory(category) => {
            if matches!(
                category.as_str(),
                "connection" | "payment" | "bug" | "general"
            ) {
                bot.send_message(
                    chat,
                    "Опишите проблему одним сообщением. Можно приложить скриншот или документ.",
                )
                .await?;
                dialogue
                    .update(State::AwaitingSupportMessage { category })
                    .await?;
            }
        }
        Action::SupportTake(id) => {
            settings.assign_support_ticket(id, uid, now_epoch());
            bot.send_message(chat, format!("✅ Обращение #{id} назначено вам."))
                .await?;
        }
        Action::SupportReply(id) => {
            if let Some(t) = settings.support_ticket(id) {
                settings.assign_support_ticket(id, uid, now_epoch());
                bot.send_message(chat, format!("Отправьте ответ пользователю {}:", t.user_id))
                    .await?;
                dialogue
                    .update(State::AwaitingSupportReply {
                        ticket_id: id,
                        user_id: t.user_id,
                    })
                    .await?;
            }
        }
        Action::SupportClose(id) => {
            if let Some(t) = settings.support_ticket(id) {
                if settings.close_support_ticket(id, uid, now_epoch()) {
                    settings.log_event(
                        now_epoch(),
                        EventKind::Support,
                        None,
                        Some(uid),
                        Some(&format!("closed ticket={id}")),
                    );
                    let _ = bot
                        .send_message(
                            ChatId(t.user_id),
                            format!("✅ Обращение #{id} закрыто. Оцените качество помощи:"),
                        )
                        .reply_markup(menu::support_rating_menu(id))
                        .await;
                    bot.send_message(chat, format!("✅ Обращение #{id} закрыто."))
                        .await?;
                }
            }
        }
        Action::SupportPriority(id, priority) => {
            if settings.set_support_priority(id, &priority, now_epoch()) {
                bot.send_message(chat, format!("✅ Приоритет обращения #{id}: {priority}"))
                    .await?;
            }
        }
        Action::SupportRate(id, rating) => {
            if settings.rate_support_ticket(id, uid, rating) {
                bot.send_message(chat, "Спасибо за оценку!")
                    .reply_markup(menu::customer_keyboard())
                    .await?;
            }
        }
        Action::FinanceExport => {
            let bytes = settings.payments_csv().into_bytes();
            bot.send_document(
                chat,
                InputFile::memory(bytes).file_name("awgram-payments.csv"),
            )
            .caption("Финансовая выгрузка CSV")
            .await?;
        }
        Action::PaymentInstructionsAsk => {
            bot.send_message(
                chat,
                format!(
                    "Текущий текст:\n\n{}\n\nОтправьте новый текст реквизитов:",
                    settings.payment_instructions()
                ),
            )
            .await?;
            dialogue.update(State::AwaitingPaymentInstructions).await?;
        }
        Action::AcquiringUrlAsk => {
            bot.send_message(chat, format!("🏦 Заготовка эквайринга\n\nУкажите URL платёжной страницы или шлюза. Доступные подстановки: {{order_id}}, {{amount}} (копейки), {{user_id}}. Обязательна {{order_id}}.\n\nПример:\nhttps://pay.example.ru/order/{{order_id}}?amount={{amount}}\n\nТекущее значение: {}\n\nДля отключения отправьте off. Для полноценного T-Банк API следующим этапом понадобятся TerminalKey, секрет и webhook.", settings.acquiring_url_template().unwrap_or_else(|| "не настроено".into()))).await?;
            dialogue.update(State::AwaitingAcquiringUrl).await?;
        }
        Action::CustomerKey(name) => {
            if settings.client_owner(&name) != Some(uid) {
                return Ok(());
            }
            let label = settings
                .device_label(&name)
                .unwrap_or_else(|| "не указано".into());
            let expiry =
                crate::vpn::model::format_expiry(lang, now_epoch(), vpn.client_expiry(&name));
            let source = settings.client_vpn_server(&name);
            let source_unavailable = source
                .as_ref()
                .is_none_or(|server| server.status != "online");
            if source_unavailable {
                let location = source
                    .as_ref()
                    .map(|server| server.location.as_str())
                    .unwrap_or("не определён");
                bot.send_message(chat, format!("❌ Нерабочее подключение\n\nКлюч: {name}\nУстройство: {label}\nСтарый сервер: {location}\nСтатус: сервер недоступен\nСрок: {expiry}\n\nКонфигурация старого сервера больше не поможет. Нажмите «Заменить нерабочий ключ», проверьте новый и подтвердите замену."))
                    .reply_markup(menu::customer_key_menu(&name))
                    .await?;
                return Ok(());
            }
            let expired = vpn
                .client_expiry(&name)
                .is_some_and(|value| value <= now_epoch());
            if expired {
                bot.send_message(chat, format!("❌ Подписка истекла\n\nВаша подписка для ключа «{name}» завершена. Продлите подписку, чтобы восстановить доступ к VPN."))
                    .reply_markup(menu::expired_subscription_menu(&name)).await?;
                return Ok(());
            }
            let status = if vpn.client_disabled(&name) {
                "отключён"
            } else {
                "активен"
            };
            bot.send_message(
                chat,
                format!("🔑 {name}\n📱 Устройство: {label}\nСтатус: {status}\nСрок: {expiry}"),
            )
            .reply_markup(menu::customer_key_menu(&name))
            .await?;
            let files = match settings.client_vpn_server(&name) {
                Some(server) if !server.is_local => {
                    nonlocal_files(&vpn, &settings, &server, &name).await
                }
                _ => vpn.existing_files(&name),
            };
            match files {
                Ok(res) => {
                    if let Err(e) = render::send_client_files(&bot, chat, lang, &res).await {
                        bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                    }
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::CustomerMove(name) => {
            if settings.client_owner(&name) != Some(uid) {
                return Ok(());
            }
            if resume_pending_replacement(&bot, chat, &vpn, &settings, lang, uid, &name).await? {
                return Ok(());
            }
            let Some(source) = settings.client_vpn_server(&name) else {
                bot.send_message(
                    chat,
                    "Не удалось определить исходный сервер этого ключа. Обратитесь в поддержку.",
                )
                .reply_markup(menu::customer_key_menu(&name))
                .await?;
                return Ok(());
            };
            if source.status == "online" {
                bot.send_message(chat, "Этот ключ находится на доступном сервере, поэтому аварийная замена для него не требуется.")
                    .reply_markup(menu::customer_key_menu(&name))
                    .await?;
                return Ok(());
            }
            let servers = settings
                .default_vpn_server()
                .and_then(|id| {
                    settings
                        .available_vpn_servers()
                        .into_iter()
                        .find(|server| server.id == id && server.id != source.id)
                })
                .into_iter()
                .collect::<Vec<_>>();
            if servers.is_empty() {
                bot.send_message(chat, "Сервер замены сейчас недоступен. Администратору нужно открыть карточку рабочего AWG-сервера и нажать «🎯 Новые ключи и замена».")
                    .reply_markup(menu::customer_key_menu(&name))
                    .await?;
                return Ok(());
            }
            bot.send_message(chat, "🛟 Безопасная замена ключа\n\nНовый ключ будет создан на рабочем сервере. Установите и проверьте его: старый ключ удалится только после вашего подтверждения. Если новый ключ не заработает, нажмите кнопку отката.")
                .reply_markup(menu::customer_move_servers_menu(&name, &servers, &settings))
                .await?;
        }
        Action::CustomerMoveServer(name, server_id) => {
            if settings.client_owner(&name) != Some(uid) {
                return Ok(());
            }
            if resume_pending_replacement(&bot, chat, &vpn, &settings, lang, uid, &name).await? {
                return Ok(());
            }
            let Some(source) = settings.client_vpn_server(&name) else {
                return Ok(());
            };
            if source.status == "online" || source.id == server_id {
                bot.send_message(chat, "Замена разрешена только для ключа с недоступного исходного сервера на другой рабочий сервер.")
                    .reply_markup(menu::customer_key_menu(&name))
                    .await?;
                return Ok(());
            }
            let Some(server) = settings.vpn_server(server_id).filter(|server| {
                server.enabled_for_provisioning
                    && settings.server_client_count(server.id) < server.capacity
            }) else {
                bot.send_message(chat, "Локация заполнена или недоступна.")
                    .await?;
                return Ok(());
            };
            let user = settings
                .user(uid)
                .ok_or_else(|| crate::error::Error::Parse("пользователь не найден".into()))?;
            let existing = settings
                .active_client_names()
                .into_iter()
                .collect::<std::collections::HashSet<_>>();
            let new_name =
                crate::vpn::validate::gen_available_names(&customer_base_name(&user), 1, &existing)
                    .map_err(|error| crate::error::Error::Parse(error.to_string()))?
                    .remove(0);
            let expiry = vpn.client_expiry(&name);
            let Some(replacement_id) =
                settings.create_key_replacement(uid, &name, &new_name, server_id, now_epoch())
            else {
                resume_pending_replacement(&bot, chat, &vpn, &settings, lang, uid, &name).await?;
                return Ok(());
            };
            let replacement = if server.is_local {
                vpn.add(&new_name, None, settings.psk_default()).await
            } else {
                nonlocal_add(&vpn, &settings, &server, &new_name).await
            };
            let replacement = match replacement {
                Ok(result) => result,
                Err(error) => {
                    settings.decide_key_replacement(replacement_id, uid, "cancelled", now_epoch());
                    return Err(error.into());
                }
            };
            if let Some(expires_at) = expiry {
                let result = if server.is_local {
                    vpn.set_client_expiry(&new_name, Some(expires_at)).await
                } else {
                    nonlocal_set_expiry(&vpn, &settings, &server, &new_name, expires_at).await
                };
                if let Err(error) = result {
                    if server.is_local {
                        let _ = vpn.remove(&new_name).await;
                    } else {
                        let _ = nonlocal_remove(&vpn, &settings, &server, &new_name).await;
                    }
                    settings.decide_key_replacement(replacement_id, uid, "cancelled", now_epoch());
                    return Err(error.into());
                }
            }
            settings.assign_client_group(&new_name, None, now_epoch());
            settings.assign_client_owner(&new_name, Some(uid));
            settings.assign_client_server(&new_name, server_id, &server.protocol);
            if !settings.retire_client(&name, now_epoch()) {
                settings.decide_key_replacement(replacement_id, uid, "cancelled", now_epoch());
                if server.is_local {
                    let _ = vpn.remove(&new_name).await;
                } else {
                    let _ = nonlocal_remove(&vpn, &settings, &server, &new_name).await;
                }
                bot.send_message(
                    chat,
                    "Не удалось скрыть старый ключ. Новый ключ удалён, повторите замену позже.",
                )
                .await?;
                return Ok(());
            }
            settings.log_event(
                now_epoch(),
                EventKind::Regen,
                Some(&new_name),
                Some(uid),
                Some(&format!("replaced={name} server={server_id}")),
            );
            bot.send_message(chat, format!("✅ Новый ключ «{new_name}» готов: {} ({}).\n\nСтарый нерабочий ключ «{name}» уже скрыт из списка. Добавьте новый ключ в приложение и проверьте подключение.", server.location, server.protocol))
                .reply_markup(menu::replacement_confirm_menu(replacement_id)).await?;
            render::send_client_files(&bot, chat, lang, &replacement).await?;
        }
        Action::CustomerMoveConfirm(id) => {
            let Some((old, new)) =
                settings.decide_key_replacement(id, uid, "confirmed", now_epoch())
            else {
                return Ok(());
            };
            let source = settings.client_vpn_server(&old);
            let source_unavailable = source
                .as_ref()
                .is_none_or(|server| server.status != "online");
            let removal = if source_unavailable {
                // The original node is unreachable by definition. Retire its
                // stale database record; there is no useful remote deletion
                // to wait for, and the old tunnel cannot authenticate on the
                // newly selected server anyway.
                settings.retire_client(&old, now_epoch());
                Ok(())
            } else {
                match source {
                    Some(server) if !server.is_local => {
                        nonlocal_remove(&vpn, &settings, &server, &old).await
                    }
                    _ => vpn.remove(&old).await,
                }
            };
            match removal {
                Ok(()) => {
                    // Remote/local collectors may run later; make the result
                    // immediately visible in the user's key list.
                    settings.retire_client(&old, now_epoch());
                    bot.send_message(
                        chat,
                        format!(
                            "✅ Подключение заменено. Новый ключ: «{new}». Старый ключ удалён."
                        ),
                    )
                    .reply_markup(menu::customer_keyboard())
                    .await?;
                }
                Err(error) => {
                    bot.send_message(chat,"Новый ключ сохранён, но старый не удалось удалить автоматически. Администратор получил уведомление.").await?;
                    for owner in &cfg.admin_ids {
                        let _ = bot
                            .send_message(
                                ChatId(*owner),
                                format!("🚨 Замена #{id}: не удалось удалить «{old}»: {error}"),
                            )
                            .await;
                    }
                }
            };
        }
        Action::CustomerMoveCancel(id) => {
            let Some((old, new)) =
                settings.decide_key_replacement(id, uid, "cancelled", now_epoch())
            else {
                return Ok(());
            };
            match settings.client_vpn_server(&new) {
                Some(server) if !server.is_local => {
                    let _ = nonlocal_remove(&vpn, &settings, &server, &new).await;
                }
                _ => {
                    let _ = vpn.remove(&new).await;
                }
            }
            settings.retire_client(&new, now_epoch());
            settings.revive_client(&old);
            bot.send_message(
                chat,
                format!("❌ Замена отменена. Старый ключ «{old}» сохранён и продолжает работать."),
            )
            .reply_markup(menu::customer_keyboard())
            .await?;
        }
        Action::CustomerRefresh(name) => {
            if settings.client_owner(&name) == Some(uid) {
                bot.send_message(chat,format!("📱 Обновление подключения «{name}»\n\nБот создаст свежие файлы под текущие настройки VPN-сервера. Криптографический ключ, VPN-адрес, владелец и срок подписки не изменятся.\n\nПосле получения удалите старое подключение из AmneziaVPN/AmneziaWG и импортируйте новое."))
                    .reply_markup(menu::customer_refresh_confirm_menu(&name))
                    .await?;
            }
        }
        Action::CustomerRefreshRun(name) => {
            if settings.client_owner(&name) != Some(uid) {
                return Ok(());
            }
            let claimed_at = now_epoch();
            if !settings.claim_client_self_refresh(&name, uid, claimed_at) {
                bot.send_message(chat,"Обновлять подключение можно не чаще одного раза в 10 минут. Попробуйте немного позже.")
                    .reply_markup(menu::customer_key_menu(&name))
                    .await?;
                return Ok(());
            }
            let waiting = bot
                .send_message(chat, "⏳ Создаю свежую конфигурацию…")
                .await
                .ok();
            let refreshed = match settings.client_vpn_server(&name) {
                Some(server) if !server.is_local => {
                    nonlocal_refresh(&vpn, &settings, &server, &name).await
                }
                _ => vpn.regen_client(&name).await,
            };
            match refreshed {
                Ok(result) => {
                    settings.log_event(
                        now_epoch(),
                        EventKind::Regen,
                        Some(&name),
                        Some(uid),
                        Some("customer self-refresh"),
                    );
                    if let Some(message) = waiting {
                        let _ = bot.delete_message(chat, message.id).await;
                    }
                    bot.send_message(chat,"✅ Подключение обновлено. Импортируйте полученные ниже файлы заново. Если проблема сохранится, обратитесь в поддержку и укажите оператора и регион.").await?;
                    if let Err(error) = render::send_client_files(&bot, chat, lang, &result).await {
                        bot.send_message(chat, i18n::error_text(lang, &error))
                            .await?;
                    }
                }
                Err(error) => {
                    settings.release_client_self_refresh(&name, uid, claimed_at);
                    if let Some(message) = waiting {
                        let _ = bot.delete_message(chat, message.id).await;
                    }
                    bot.send_message(chat, i18n::error_text(lang, &error))
                        .reply_markup(menu::customer_key_menu(&name))
                        .await?;
                }
            }
        }
        Action::DeviceLabelAsk(name) => {
            if settings.client_owner(&name) == Some(uid) {
                bot.send_message(chat, format!("Введите название устройства для ключа {name}, например «iPhone» или «Ноутбук»:" )).await?;
                dialogue.update(State::AwaitingDeviceLabel { name }).await?;
            }
        }
        Action::Menu => {
            dialogue.update(State::Idle).await?;
            match &role {
                Role::GroupAdmin(groups) => match current_ga_group(&settings, uid, groups) {
                    Some(gid) => {
                        let gname = settings.group(gid).map(|g| g.name).unwrap_or_default();
                        edit_or_send(
                            &bot,
                            chat,
                            msg_id,
                            i18n::ga_menu_title(lang, &gname),
                            menu::ga_main_menu(lang, groups.len() > 1),
                        )
                        .await;
                    }
                    None => {
                        show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    }
                },
                Role::Owner => {
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        "🏠 Админ-панель".to_string(),
                        menu::admin_dashboard_menu(),
                    )
                    .await;
                }
                _ => {
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        i18n::menu_title(lang),
                        menu::main_menu(lang),
                    )
                    .await;
                }
            }
        }
        Action::GroupSelectMenu => {
            if let Role::GroupAdmin(groups) = &role {
                show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
            }
        }
        Action::GroupSelect(id) => {
            // Принадлежность группы роли уже проверена в authorize —
            // здесь `groups` нужен только для рендера (groups.len()).
            if let Role::GroupAdmin(groups) = &role {
                settings.set_current_group(uid, id);
                let gname = settings.group(id).map(|g| g.name).unwrap_or_default();
                edit_or_send(
                    &bot,
                    chat,
                    msg_id,
                    i18n::ga_menu_title(lang, &gname),
                    menu::ga_main_menu(lang, groups.len() > 1),
                )
                .await;
            }
        }
        Action::List => {
            // Экран списка: stats → filter+sort (фильтр из настроек) → скоуп роли → рендер.
            // Фильтр/сортировку/скоуп см. render_clients_list/scope_for.
            let scope = match scope_for(&role, &settings, uid) {
                Some(s) => s,
                None => {
                    if let Role::GroupAdmin(groups) = &role {
                        show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    }
                    return Ok(());
                }
            };
            render_clients_list(
                &bot,
                chat,
                msg_id,
                lang,
                &vpn,
                &settings,
                uid,
                0,
                scope,
                home_menu(&role, lang),
                role.is_owner(),
            )
            .await;
        }
        Action::Page(p) => {
            // Пагинация: тот же рендер, но страница p. Фильтр из настроек —
            // переживает навигацию по страницам (Action::Page его не меняет).
            let scope = match scope_for(&role, &settings, uid) {
                Some(s) => s,
                None => {
                    if let Role::GroupAdmin(groups) = &role {
                        show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    }
                    return Ok(());
                }
            };
            render_clients_list(
                &bot,
                chat,
                msg_id,
                lang,
                &vpn,
                &settings,
                uid,
                p,
                scope,
                home_menu(&role, lang),
                role.is_owner(),
            )
            .await;
        }
        Action::Stats => {
            let scope = match scope_for(&role, &settings, uid) {
                Some(s) => s,
                None => {
                    if let Role::GroupAdmin(groups) = &role {
                        show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    }
                    return Ok(());
                }
            };
            match managed_clients(&vpn, &settings).await {
                Ok(mut clients) => {
                    clients.retain(|c| scope.admits(settings.client_group(&c.name)));
                    let now = now_epoch();
                    // Суммарный трафик: All — глобальный одним запросом,
                    // иначе — сумма пер-клиентских сводок отфильтрованного списка.
                    let summary = if scope == ListScope::All {
                        settings.traffic_summary(None, now)
                    } else {
                        let mut acc = crate::store::TrafficSummary::default();
                        for c in &clients {
                            acc.add(&settings.traffic_summary(Some(&c.name), now));
                        }
                        acc
                    };
                    let top = if scope == ListScope::All {
                        settings.top_clients(7, 5, now)
                    } else {
                        let names: std::collections::HashSet<&str> =
                            clients.iter().map(|c| c.name.as_str()).collect();
                        settings
                            .top_clients(7, 10_000, now)
                            .into_iter()
                            .filter(|(n, _)| names.contains(n.as_str()))
                            .take(5)
                            .collect()
                    };
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        format_stats(lang, &clients, now, &summary, &top),
                        if role.is_owner() {
                            menu::statistics_menu()
                        } else {
                            home_menu(&role, lang)
                        },
                    )
                    .await;
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::ShowClient(name) => match managed_clients(&vpn, &settings).await {
            Ok(clients) => match clients.iter().find(|c| c.name == name) {
                Some(c) => {
                    let now = now_epoch();
                    let expiry = vpn.client_expiry(&name);
                    let traffic = settings.traffic_summary(Some(&name), now);
                    let group_line = settings
                        .client_group(&name)
                        .and_then(|gid| settings.group(gid))
                        .map(|g| i18n::group_label_line(lang, &g.name))
                        .unwrap_or_default();
                    let crm_line = if role.is_owner() {
                        format!(
                            "\n👤 Владелец: {}\n📝 Заметка: {}",
                            settings
                                .client_owner(&name)
                                .map(|v| v.to_string())
                                .unwrap_or_else(|| "не назначен".into()),
                            settings.client_note(&name).unwrap_or_else(|| "—".into())
                        )
                    } else {
                        String::new()
                    };
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        format!(
                            "{}{}{}",
                            format_client_card(lang, c, now, expiry, &traffic),
                            group_line,
                            crm_line
                        ),
                        menu::client_card(lang, &name, role.is_owner()),
                    )
                    .await;
                }
                None => {
                    bot.send_message(chat, i18n::not_found(lang)).await?;
                }
            },
            Err(e) => {
                bot.send_message(chat, i18n::error_text(lang, &e)).await?;
            }
        },
        Action::ClientHistory(name) => {
            let now = now_epoch();
            let events = settings.client_events(&name, 10);
            edit_or_send(
                &bot,
                chat,
                msg_id,
                render::format_client_history(lang, &name, &events, now),
                menu::client_history(lang, &name),
            )
            .await;
        }
        Action::SendConf(name) => {
            // 📄 Конфиг — только .conf, без QR/ссылки (фильтр выдачи не применяется:
            // это ручная повторная выдача конкретного артефакта).
            match client_files(&vpn, &settings, &name).await {
                Ok(res) => {
                    if let Err(e) = bot
                        .send_document(chat, InputFile::file(&res.conf_path))
                        .await
                    {
                        let err = crate::error::Error::Telegram(e.to_string());
                        bot.send_message(chat, i18n::error_text(lang, &err)).await?;
                    }
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::SendQr(name) => {
            // 🖼 QR — опционален (qrencode может отсутствовать на сервере).
            match client_files(&vpn, &settings, &name).await {
                Ok(res) if std::path::Path::new(&res.qr_path).exists() => {
                    if let Err(e) = bot.send_photo(chat, InputFile::file(&res.qr_path)).await {
                        let err = crate::error::Error::Telegram(e.to_string());
                        bot.send_message(chat, i18n::error_text(lang, &err)).await?;
                    }
                }
                Ok(_) => {
                    bot.send_message(chat, i18n::qr_not_generated(lang)).await?;
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::SendLink(name) => {
            // 🔗 Ссылка vpn:// — опциональна (qrencode генерирует её заодно с QR).
            match client_files(&vpn, &settings, &name).await {
                Ok(res) if !res.uri.is_empty() => {
                    bot.send_message(chat, i18n::import_link(lang, &res.uri))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Ok(_) => {
                    bot.send_message(chat, i18n::link_unavailable(lang)).await?;
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::SendAll(name) => {
            // 📦 Всё — безусловная выдача conf+QR+ссылка (фильтр настроек игнорируется:
            // пользователь явно запросил всё через карточку клиента).
            match client_files(&vpn, &settings, &name).await {
                Ok(res) => {
                    if let Err(e) = render::send_client_files(&bot, chat, lang, &res).await {
                        bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                    }
                }
                Err(e) => {
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::AskDelete(name) => {
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::confirm_delete(lang, &name),
                menu::confirm_delete(lang, &name),
            )
            .await;
        }
        Action::ConfirmDelete(name) => match client_remove(&vpn, &settings, &name).await {
            Ok(()) => {
                settings.log_event(
                    now_epoch(),
                    EventKind::ClientRemove,
                    Some(&name),
                    Some(uid),
                    None,
                );
                bot.send_message(chat, i18n::deleted(lang, &name))
                    .reply_markup(home_menu(&role, lang))
                    .parse_mode(ParseMode::Html)
                    .await?;
            }
            Err(e) => {
                tracing::error!(error = %e, "remove провалился");
                bot.send_message(chat, i18n::error_text(lang, &e)).await?;
            }
        },
        Action::Recreate(name) => {
            bot.send_message(chat, i18n::ask_expiry(lang))
                .reply_markup(menu::expiry_menu(lang))
                .await?;
            dialogue
                .update(State::AwaitingExpiry {
                    name,
                    recreate: true,
                })
                .await?;
        }
        Action::Regen(name) => {
            let waiting = bot.send_message(chat, i18n::regen_running(lang)).await.ok();
            match client_refresh(&vpn, &settings, &name).await {
                Ok(res) => {
                    settings.log_event(now_epoch(), EventKind::Regen, Some(&name), Some(uid), None);
                    if let Err(e) = render::send_client_files(&bot, chat, lang, &res).await {
                        tracing::error!(error = %e, "не удалось отправить файлы после regen");
                        bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                    } else {
                        bot.send_message(chat, i18n::done(lang))
                            .reply_markup(home_menu(&role, lang))
                            .parse_mode(ParseMode::Html)
                            .await?;
                    }
                }
                Err(e) => {
                    tracing::error!(error = %e, "regen провалился");
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
        }
        Action::RegenAll => {
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::confirm_regen_all(lang),
                menu::confirm_regen_all(lang),
            )
            .await;
        }
        Action::RegenAllRun(reset_routes) => {
            let waiting = bot
                .send_message(chat, i18n::regen_all_running(lang))
                .await
                .ok();
            let regen_all_result = vpn.regen_all(reset_routes).await;
            if regen_all_result.is_ok() {
                settings.log_event(now_epoch(), EventKind::RegenAll, None, Some(uid), None);
            }
            match regen_all_result {
                Ok(crate::vpn::RegenAllOutcome::NoClients) => {
                    bot.send_message(chat, i18n::clients_empty(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Ok(crate::vpn::RegenAllOutcome::Done(_n)) => {
                    bot.send_message(chat, i18n::regen_all_done(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Ok(crate::vpn::RegenAllOutcome::Partial { .. }) => {
                    bot.send_message(chat, i18n::regen_all_partial(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "массовый regen провалился");
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
        }
        Action::Add => {
            // Групповой админ без выбранной группы не может создавать «в никуда» —
            // сначала экран выбора группы (та же логика, что и в List).
            if let Role::GroupAdmin(groups) = &role {
                if current_ga_group(&settings, uid, groups).is_none() {
                    show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    return Ok(());
                }
            }
            bot.send_message(chat, i18n::ask_client_name(lang, false))
                .await?;
            dialogue.update(State::AwaitingName).await?;
        }
        Action::Expiry(kind) => {
            let (name, recreate) = match dialogue.get().await?.unwrap_or_default() {
                State::AwaitingExpiry { name, recreate } => (name, recreate),
                _ => {
                    bot.send_message(chat, session_expired_text(lang))
                        .reply_markup(home_menu(&role, lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    return Ok(());
                }
            };
            if kind == "custom" {
                bot.send_message(chat, i18n::ask_custom_expiry(lang))
                    .await?;
                dialogue
                    .update(State::AwaitingCustomExpiry { name, recreate })
                    .await?;
            } else {
                let expires = if kind == "none" {
                    None
                } else {
                    Some(kind.clone())
                };
                bot.send_message(chat, i18n::psk_step(lang, settings.psk_default()))
                    .reply_markup(menu::psk_step(lang, settings.psk_default()))
                    .parse_mode(ParseMode::Html)
                    .await?;
                dialogue
                    .update(State::AwaitingPsk {
                        name,
                        expires,
                        recreate,
                    })
                    .await?;
            }
        }
        Action::AddPsk(psk) => {
            let (name, expires, recreate) = match dialogue.get().await?.unwrap_or_default() {
                State::AwaitingPsk {
                    name,
                    expires,
                    recreate,
                } => (name, expires, recreate),
                _ => {
                    bot.send_message(chat, session_expired_text(lang))
                        .reply_markup(home_menu(&role, lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    return Ok(());
                }
            };
            // Recreate: право на объект проверялось только на входе в
            // Action::Recreate — за время диалога (выбор срока/PSK) владелец
            // мог отозвать группу у админа или перенести клиента в другую
            // группу. Перепроверяем непосредственно перед finish_add.
            if recreate && !client_in_scope(&role, &settings, &name) {
                bot.send_message(chat, session_expired_text(lang))
                    .reply_markup(home_menu(&role, lang))
                    .parse_mode(ParseMode::Html)
                    .await?;
                dialogue.exit().await?;
                return Ok(());
            }
            // Группа для привязки: при recreate — существующая привязка
            // клиента (см. group_for_new_client); новому клиенту групповому
            // админу — его текущая группа (если она стала недоступна за время
            // диалога — не создаём «в никуда», отправляем на выбор группы),
            // владельцу — без группы.
            let group = match group_for_new_client(&role, &settings, uid, recreate, &name) {
                Some(g) => g,
                None => {
                    if let Role::GroupAdmin(groups) = &role {
                        show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    }
                    dialogue.exit().await?;
                    return Ok(());
                }
            };
            if recreate {
                let Some(server) = settings.client_vpn_server(&name) else {
                    bot.send_message(chat, "Не удалось определить сервер существующего ключа.")
                        .reply_markup(home_menu(&role, lang))
                        .await?;
                    dialogue.exit().await?;
                    return Ok(());
                };
                finish_add(
                    &bot,
                    chat,
                    &vpn,
                    &settings,
                    lang,
                    &name,
                    expires.as_deref(),
                    psk,
                    true,
                    uid,
                    group,
                    &role,
                    &server,
                )
                .await;
                dialogue.exit().await?;
            } else {
                let servers = settings.available_vpn_servers();
                if servers.is_empty() {
                    bot.send_message(chat, "Нет подключённых AWG-серверов, доступных для создания ключа. Проверьте панель и включите выдачу в карточке сервера.")
                        .reply_markup(home_menu(&role, lang))
                        .await?;
                    dialogue.exit().await?;
                    return Ok(());
                }
                bot.send_message(chat, "🌍 Выберите сервер, на котором нужно создать ключ:")
                    .reply_markup(menu::add_server_menu(&servers))
                    .await?;
                dialogue
                    .update(State::AwaitingAddServer {
                        name,
                        expires,
                        psk,
                        group,
                    })
                    .await?;
            }
        }
        Action::AddServer(server_id) => {
            let State::AwaitingAddServer {
                name,
                expires,
                psk,
                group,
            } = dialogue.get().await?.unwrap_or_default()
            else {
                bot.send_message(chat, session_expired_text(lang))
                    .reply_markup(home_menu(&role, lang))
                    .await?;
                return Ok(());
            };
            let Some(server) = settings
                .available_vpn_servers()
                .into_iter()
                .find(|server| server.id == server_id)
            else {
                bot.send_message(chat, "Сервер больше не доступен для создания ключей.")
                    .reply_markup(home_menu(&role, lang))
                    .await?;
                dialogue.exit().await?;
                return Ok(());
            };
            finish_add(
                &bot,
                chat,
                &vpn,
                &settings,
                lang,
                &name,
                expires.as_deref(),
                psk,
                false,
                uid,
                group,
                &role,
                &server,
            )
            .await;
            dialogue.exit().await?;
        }
        Action::AddBulk => {
            if let Role::GroupAdmin(groups) = &role {
                if current_ga_group(&settings, uid, groups).is_none() {
                    show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    return Ok(());
                }
            }
            // Шаг 1/4 массового диалога: запрос префикса (текстовый ввод, а не
            // кнопка). Валидация префикса — на следующем шаге (gen_bulk_names с
            // count=1 как smoke-проверка), тут только приглашение к вводу.
            bot.send_message(chat, i18n::ask_bulk_prefix(lang)).await?;
            dialogue.update(State::AwaitingBulkPrefix).await?;
        }
        Action::AddBulkRun(count) => {
            // callback_data — untrusted input (craftable). Клавиатура эмитит
            // только 1/3/5/10, но защищаемся от crafted bulk:N извне.
            if count == 0 || count > crate::vpn::validate::MAX_BULK as usize {
                bot.send_message(chat, session_expired_text(lang))
                    .reply_markup(menu::main_menu(lang))
                    .parse_mode(ParseMode::Html)
                    .await?;
                return Ok(());
            }
            // Шаг 2/4: префикс уже введён (AwaitingBulkCount хранит его) —
            // переходим к выбору срока. Кол-во пришло из кнопки bulk_count_menu
            // (1/3/5/10 — префикс уже валиден, max=MAX_BULK держит клавиатура).
            let prefix = match dialogue.get().await?.unwrap_or_default() {
                State::AwaitingBulkCount { prefix } => prefix,
                _ => {
                    bot.send_message(chat, session_expired_text(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    return Ok(());
                }
            };
            bot.send_message(chat, i18n::ask_expiry(lang))
                .reply_markup(menu::bulk_expiry_menu(lang))
                .await?;
            dialogue
                .update(State::AwaitingBulkExpiry { prefix, count })
                .await?;
        }
        Action::BulkExpiry(kind) => {
            // Шаг 3/4: срок выбран. «custom» → текстовый ввод срока,
            // иначе — переход к выбору PSK с уже готовым expires.
            let (prefix, count) = match dialogue.get().await?.unwrap_or_default() {
                State::AwaitingBulkExpiry { prefix, count } => (prefix, count),
                _ => {
                    bot.send_message(chat, session_expired_text(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    return Ok(());
                }
            };
            if kind == "custom" {
                bot.send_message(chat, i18n::ask_custom_expiry(lang))
                    .await?;
                dialogue
                    .update(State::AwaitingBulkCustomExpiry { prefix, count })
                    .await?;
            } else {
                let expires = if kind == "none" {
                    None
                } else {
                    Some(kind.clone())
                };
                bot.send_message(chat, i18n::psk_step(lang, settings.psk_default()))
                    .reply_markup(menu::bulk_psk_step(lang, settings.psk_default()))
                    .parse_mode(ParseMode::Html)
                    .await?;
                dialogue
                    .update(State::AwaitingBulkPsk {
                        prefix,
                        count,
                        expires,
                    })
                    .await?;
            }
        }
        Action::AddBulkPsk(psk) => {
            // Шаг 4/4: PSK выбран — финальный забег (превентивные проверки +
            // add_many + альбом). После finish_bulk диалог закрывается.
            let (prefix, count, expires) = match dialogue.get().await?.unwrap_or_default() {
                State::AwaitingBulkPsk {
                    prefix,
                    count,
                    expires,
                } => (prefix, count, expires),
                _ => {
                    bot.send_message(chat, session_expired_text(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                    return Ok(());
                }
            };
            let servers = settings.available_vpn_servers();
            if servers.is_empty() {
                bot.send_message(chat, "Нет доступных AWG-серверов для создания ключей.")
                    .await?;
                dialogue.exit().await?;
            } else {
                bot.send_message(chat, "📍 Последний шаг · Выберите сервер для всей пачки:")
                    .reply_markup(menu::bulk_servers_menu(&servers))
                    .await?;
                dialogue
                    .update(State::AwaitingBulkServer {
                        prefix,
                        count,
                        expires,
                        psk,
                    })
                    .await?;
            }
        }
        Action::BulkServer(server_id) => {
            let State::AwaitingBulkServer {
                prefix,
                count,
                expires,
                psk,
            } = dialogue.get().await?.unwrap_or_default()
            else {
                bot.send_message(chat, session_expired_text(lang)).await?;
                return Ok(());
            };
            let Some(server) = settings
                .available_vpn_servers()
                .into_iter()
                .find(|server| server.id == server_id)
            else {
                bot.send_message(chat, "Выбранный сервер недоступен или заполнен.")
                    .await?;
                return Ok(());
            };
            let group = match &role {
                Role::GroupAdmin(groups) => current_ga_group(&settings, uid, groups),
                Role::Owner => match settings.owner_scope(uid) {
                    ListScope::Group(id) => Some(id),
                    _ => None,
                },
                _ => None,
            };
            finish_bulk(
                &bot,
                chat,
                &vpn,
                &settings,
                lang,
                &prefix,
                count,
                expires.as_deref(),
                psk,
                uid,
                group,
                &server,
            )
            .await;
            dialogue.exit().await?;
        }
        Action::Settings => {
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::Modify(name) => {
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::modify_param_select_title(lang),
                menu::modify_param_menu(lang, &name),
            )
            .await;
            dialogue.update(State::AwaitingModifyParam { name }).await?;
        }
        Action::ModifyParam(name, param) => {
            bot.send_message(chat, i18n::ask_modify_param(lang, param))
                .await?;
            dialogue
                .update(State::AwaitingModifyValue { name, param })
                .await?;
        }
        Action::Restart => {
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::confirm_restart(lang),
                menu::confirm_restart_menu(lang),
            )
            .await;
        }
        Action::RestartRun => {
            let waiting = bot.send_message(chat, i18n::creating(lang)).await.ok();
            match vpn.restart().await {
                Ok(out) => {
                    settings.log_event(now_epoch(), EventKind::Restart, None, Some(uid), None);
                    if let Some(m) = waiting {
                        let _ = bot.delete_message(chat, m.id).await;
                    }
                    let verification = if !out.active {
                        "⚠️ Systemd-служба неактивна после перезапуска.".to_string()
                    } else {
                        match vpn.check().await {
                            Ok(report) if report.ok => {
                                "✅ Контрольная проверка VPN пройдена.".to_string()
                            }
                            Ok(_) => "⚠️ Служба запущена, но контрольная проверка обнаружила проблемы. Откройте подробную диагностику.".to_string(),
                            Err(error) => {
                                tracing::warn!(%error, "контрольная проверка после restart провалилась");
                                "⚠️ Не удалось выполнить контрольную проверку. Откройте подробную диагностику.".to_string()
                            }
                        }
                    };
                    bot.send_message(
                        chat,
                        format!("{}\n\n{verification}", i18n::restart_done(lang, out.active)),
                    )
                    .reply_markup(if role.is_owner() {
                        menu::vpn_service_menu()
                    } else {
                        menu::main_menu(lang)
                    })
                    .parse_mode(ParseMode::Html)
                    .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "restart провалился");
                    if let Some(m) = waiting {
                        let _ = bot.delete_message(chat, m.id).await;
                    }
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::RepairModule => {
            let waiting = bot.send_message(chat, i18n::creating(lang)).await.ok();
            match vpn.repair_module().await {
                Ok(out) => {
                    settings.log_event(now_epoch(), EventKind::Repair, None, Some(uid), None);
                    if let Some(m) = waiting {
                        let _ = bot.delete_message(chat, m.id).await;
                    }
                    bot.send_message(chat, i18n::repair_result(lang, out.rc))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "repair-module провалился");
                    if let Some(m) = waiting {
                        let _ = bot.delete_message(chat, m.id).await;
                    }
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
        }
        Action::Lang(code) => {
            if let Some(l) = i18n::parse_lang(&code) {
                settings.set_lang(uid, l);
            }
            let lang = settings.lang(uid);
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::menu_title(lang),
                menu::main_menu(lang),
            )
            .await;
        }
        Action::SetLang(code) => {
            if let Some(l) = i18n::parse_lang(&code) {
                settings.set_lang(uid, l);
            }
            let lang = settings.lang(uid);
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::SetPsk(on) => {
            settings.set_psk_default(on);
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::SetSlug(on) => {
            settings.set_name_slug(on);
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::SetConf(on) => {
            settings.set_deliver_conf(on);
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::SetQr(on) => {
            settings.set_deliver_qr(on);
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::SetLink(on) => {
            settings.set_deliver_link(on);
            show_settings(&bot, chat, msg_id, lang, &settings).await;
        }
        Action::SetListFilter(f) => {
            // Фильтр — персональная настройка: групповой админ, переключая свой
            // список, не меняет вид владельцу (и наоборот). Скоуп (какие
            // клиенты вообще видны) считается отдельно через scope_for,
            // как в List/Page.
            settings.set_client_filter(uid, f);
            let scope = match scope_for(&role, &settings, uid) {
                Some(s) => s,
                None => {
                    if let Role::GroupAdmin(groups) = &role {
                        show_group_select(&bot, chat, msg_id, lang, &settings, groups).await;
                    }
                    return Ok(());
                }
            };
            // Сохраняем фильтр персистентно, затем перерисовываем список с
            // НУЛЕВОЙ страницей — содержимое сменилось, старая страница могла
            // стать невалидной (напр. был на стр.2 оффлайн, переключил на онлайн).
            render_clients_list(
                &bot,
                chat,
                msg_id,
                lang,
                &vpn,
                &settings,
                uid,
                0,
                scope,
                home_menu(&role, lang),
                role.is_owner(),
            )
            .await;
        }
        Action::Backup => {
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::backup_menu_title(lang),
                menu::backup_menu(lang),
            )
            .await;
        }
        Action::BackupNew => {
            let waiting = bot
                .send_message(chat, i18n::backup_creating(lang))
                .await
                .ok();
            match vpn.backup().await {
                Ok(bf) => {
                    settings.log_event(now_epoch(), EventKind::Backup, None, Some(uid), None);
                    // Свежесозданный бэкап — самый новый по mtime, т.е. индекс 0 в list_backups().
                    bot.send_message(chat, i18n::backup_done(lang, &bf.name))
                        .reply_markup(menu::backup_card(lang, 0))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "backup провалился");
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
        }
        Action::BackupList => match vpn.list_backups() {
            Ok(list) if list.is_empty() => {
                edit_or_send(
                    &bot,
                    chat,
                    msg_id,
                    i18n::backups_empty(lang),
                    menu::main_menu(lang),
                )
                .await;
            }
            Ok(list) => {
                edit_or_send(
                    &bot,
                    chat,
                    msg_id,
                    i18n::backups_list_title(lang),
                    menu::backups_list(lang, &list),
                )
                .await;
            }
            Err(e) => {
                bot.send_message(chat, i18n::error_text(lang, &e)).await?;
            }
        },
        Action::BackupCard(idx) => match vpn.list_backups() {
            Ok(list) => match list.get(idx) {
                Some(bf) => {
                    let text = format!("<code>{}</code>", i18n::html_escape(&bf.name));
                    edit_or_send(&bot, chat, msg_id, text, menu::backup_card(lang, idx)).await;
                }
                None => {
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        i18n::backup_not_found(lang),
                        menu::main_menu(lang),
                    )
                    .await;
                }
            },
            Err(e) => {
                bot.send_message(chat, i18n::error_text(lang, &e)).await?;
            }
        },
        Action::BackupDownload(idx) => match vpn.list_backups() {
            Ok(list) => match list.get(idx) {
                Some(bf) => {
                    if let Err(e) = bot.send_document(chat, InputFile::file(&bf.path)).await {
                        tracing::error!(error = %e, "send_document провалился");
                        let err = crate::error::Error::Telegram(e.to_string());
                        bot.send_message(chat, i18n::error_text(lang, &err)).await?;
                    }
                }
                None => {
                    bot.send_message(chat, i18n::backup_not_found(lang))
                        .reply_markup(menu::main_menu(lang))
                        .await?;
                }
            },
            Err(e) => {
                bot.send_message(chat, i18n::error_text(lang, &e)).await?;
            }
        },
        Action::Restore(idx) => match vpn.list_backups() {
            Ok(list) => match list.get(idx) {
                Some(bf) => {
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        i18n::confirm_restore(lang, &bf.name),
                        menu::confirm_restore(lang, idx),
                    )
                    .await;
                }
                None => {
                    edit_or_send(
                        &bot,
                        chat,
                        msg_id,
                        i18n::backup_not_found(lang),
                        menu::main_menu(lang),
                    )
                    .await;
                }
            },
            Err(e) => {
                bot.send_message(chat, i18n::error_text(lang, &e)).await?;
            }
        },
        Action::RestoreYes(idx) => {
            let waiting = bot.send_message(chat, i18n::restoring(lang)).await.ok();
            match vpn.restore(idx).await {
                Ok(()) => {
                    settings.log_event(now_epoch(), EventKind::Restore, None, Some(uid), None);
                    bot.send_message(chat, i18n::restore_done(lang))
                        .reply_markup(menu::main_menu(lang))
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "restore провалился");
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
        }
        Action::Check => {
            let waiting = bot.send_message(chat, i18n::check_running(lang)).await.ok();
            match vpn.check().await {
                Ok(report) => {
                    let body = i18n::check_card(lang, &report);
                    bot.send_message(chat, body)
                        .parse_mode(ParseMode::Html)
                        .reply_markup(if role.is_owner() {
                            menu::vpn_service_menu()
                        } else {
                            menu::main_menu(lang)
                        })
                        .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "check провалился");
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
        }
        Action::Diagnose => {
            let waiting = bot
                .send_message(chat, i18n::diagnose_running(lang))
                .await
                .ok();
            match vpn.diagnose().await {
                Ok(body) => {
                    let body = truncate_for_message(body);
                    bot.send_message(chat, i18n::diagnose_result(lang, &body))
                        .reply_markup(if role.is_owner() {
                            menu::vpn_service_menu()
                        } else {
                            menu::main_menu(lang)
                        })
                        .parse_mode(ParseMode::Html)
                        .await?;
                }
                Err(e) => {
                    tracing::error!(error = %e, "diagnose провалился");
                    bot.send_message(chat, i18n::error_text(lang, &e)).await?;
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
        }
        Action::Groups => {
            let groups: Vec<(crate::store::GroupRow, i64)> = settings
                .list_groups()
                .into_iter()
                .map(|g| {
                    let n = settings.group_client_count(g.id);
                    (g, n)
                })
                .collect();
            let title = if groups.is_empty() {
                i18n::groups_empty(lang)
            } else {
                i18n::groups_title(lang, groups.len())
            };
            edit_or_send(&bot, chat, msg_id, title, menu::groups_menu(lang, &groups)).await;
        }
        Action::GroupCreate => {
            bot.send_message(chat, i18n::ask_group_name(lang)).await?;
            dialogue.update(State::AwaitingGroupName).await?;
        }
        Action::GroupCard(id) => {
            show_group_card(&bot, chat, msg_id, lang, &settings, id).await;
        }
        Action::GroupRenameAsk(id) => {
            bot.send_message(chat, i18n::ask_group_name(lang)).await?;
            dialogue.update(State::AwaitingGroupRename { id }).await?;
        }
        Action::GroupQuotaAsk(id) => {
            bot.send_message(chat, i18n::ask_group_quota(lang)).await?;
            dialogue.update(State::AwaitingGroupQuota { id }).await?;
        }
        Action::GroupAdmins(id) => {
            let admins = settings.group_admin_ids(id);
            let name = settings.group(id).map(|g| g.name).unwrap_or_default();
            let title = if admins.is_empty() {
                i18n::group_admins_empty(lang)
            } else {
                i18n::group_admins_title(lang, &name)
            };
            edit_or_send(
                &bot,
                chat,
                msg_id,
                title,
                menu::group_admins_menu(lang, id, &admins),
            )
            .await;
        }
        Action::GroupAdminRemove(id, admin_uid) => {
            settings.remove_group_admin(id, admin_uid);
            settings.log_event(
                now_epoch(),
                EventKind::AdminRemove,
                None,
                Some(uid),
                Some(&format!("group={id} user={admin_uid}")),
            );
            bot.send_message(chat, i18n::admin_removed(lang, admin_uid))
                .await?;
            show_group_card(&bot, chat, msg_id, lang, &settings, id).await;
        }
        Action::GroupDeleteAsk(id) => {
            let name = settings.group(id).map(|g| g.name).unwrap_or_default();
            let count = settings.group_client_count(id);
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::group_delete_choice(lang, &name, count),
                menu::group_delete_choice_menu(lang, id),
            )
            .await;
        }
        Action::GroupDeleteDetach(id) => {
            let name = settings.group(id).map(|g| g.name).unwrap_or_default();
            settings.delete_group(id);
            settings.log_event(
                now_epoch(),
                EventKind::GroupDelete,
                None,
                Some(uid),
                Some(&format!("detach {name}")),
            );
            bot.send_message(chat, i18n::group_deleted(lang, &name))
                .parse_mode(ParseMode::Html)
                .reply_markup(menu::main_menu(lang))
                .await?;
        }
        Action::GroupDeleteAllAsk(id) => {
            let name = settings.group(id).map(|g| g.name).unwrap_or_default();
            let count = settings.group_client_count(id);
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::confirm_delete_group_clients(lang, &name, count),
                menu::confirm_group_delete_clients_menu(lang, id),
            )
            .await;
        }
        Action::GroupDeleteAllYes(id) => {
            let name = settings.group(id).map(|g| g.name).unwrap_or_default();
            let clients = settings.group_client_names(id);
            let waiting = bot
                .send_message(chat, i18n::group_delete_running(lang))
                .await
                .ok();
            let mut failed = 0usize;
            for c in &clients {
                match client_remove(&vpn, &settings, c).await {
                    Ok(()) => {
                        settings.log_event(
                            now_epoch(),
                            EventKind::ClientRemove,
                            Some(c),
                            Some(uid),
                            Some("group_delete"),
                        );
                    }
                    Err(e) => {
                        failed += 1;
                        tracing::error!(error = %e, client = %c, "remove при удалении группы провалился");
                    }
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            if failed == 0 {
                settings.delete_group(id);
                settings.log_event(
                    now_epoch(),
                    EventKind::GroupDelete,
                    None,
                    Some(uid),
                    Some(&format!("with_clients {name}")),
                );
                bot.send_message(chat, i18n::group_deleted(lang, &name))
                    .parse_mode(ParseMode::Html)
                    .reply_markup(menu::main_menu(lang))
                    .await?;
            } else {
                // Часть клиентов не удалилась — группу не трогаем, чтобы не
                // потерять привязку выживших. Владелец повторит после починки.
                let err = crate::error::Error::Telegram(format!("{failed} clients not removed"));
                bot.send_message(chat, i18n::error_text(lang, &err)).await?;
            }
        }
        Action::GroupInvite(id) => {
            let first_admin_ever = !settings.has_any_group_admin();
            let Some(token) = settings.create_invite(id, uid, now_epoch()) else {
                // Ошибка БД: ссылки нет — честная ошибка вместо «успеха»
                // с мёртвым токеном.
                let err = crate::error::Error::Telegram("db".into());
                bot.send_message(chat, i18n::error_text(lang, &err)).await?;
                return Ok(());
            };
            settings.log_event(
                now_epoch(),
                EventKind::InviteCreate,
                None,
                Some(uid),
                Some(&format!("group={id}")),
            );
            let me = bot.get_me().await?;
            let username = me.username.clone().unwrap_or_default();
            let url = format!("https://t.me/{username}?start=inv_{token}");
            let hours = crate::store::INVITE_TTL_SECS / 3600;
            bot.send_message(chat, i18n::invite_link_text(lang, &url, hours))
                .parse_mode(ParseMode::Html)
                .await?;
            let _ = first_admin_ever;
            show_group_card(&bot, chat, msg_id, lang, &settings, id).await;
        }
        Action::GroupInviteRevoke(id) => {
            settings.revoke_invite(id);
            settings.log_event(
                now_epoch(),
                EventKind::InviteRevoke,
                None,
                Some(uid),
                Some(&format!("group={id}")),
            );
            bot.send_message(chat, i18n::invite_revoked(lang)).await?;
            show_group_card(&bot, chat, msg_id, lang, &settings, id).await;
        }
        Action::GroupAdminById(id) => {
            bot.send_message(chat, i18n::ask_admin_id(lang)).await?;
            dialogue.update(State::AwaitingGroupAdminId { id }).await?;
        }
        Action::MoveClientAsk(name) => {
            let groups = settings.list_groups();
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::move_client_title(lang, &name),
                menu::move_client_menu(lang, &name, &groups),
            )
            .await;
        }
        Action::MoveClientTo(target, name) => {
            // Целевая группа могла исчезнуть между показом меню (Task 13
            // ревью, Important) и кликом (другой владелец удалил её) — без
            // этой проверки assign_client_group молча пишет висячий
            // group_id (FK не включены), а client_moved соврал бы, что
            // клиент отвязан от группы.
            let gname = if let Some(id) = target {
                let Some(g) = settings.group(id) else {
                    bot.send_message(chat, i18n::not_found(lang)).await?;
                    return Ok(());
                };
                // Квота действует и на перенос: полная группа не принимает клиентов
                // (владелец сначала поднимает лимит). Проверка и привязка — атомарно
                // в store (TOCTOU-фикс); no-op переноса в свою же группу разрешён там же.
                match settings.try_assign_client_group(&name, id, now_epoch()) {
                    crate::store::QuotaAssign::Assigned => {}
                    crate::store::QuotaAssign::Full => {
                        let quota = g.max_clients.unwrap_or(0);
                        bot.send_message(chat, i18n::quota_reached(lang, quota))
                            .await?;
                        return Ok(());
                    }
                    crate::store::QuotaAssign::Db => {
                        let err = crate::error::Error::Telegram("db".into());
                        bot.send_message(chat, i18n::error_text(lang, &err)).await?;
                        return Ok(());
                    }
                }
                Some(g.name)
            } else {
                settings.assign_client_group(&name, None, now_epoch());
                None
            };
            bot.send_message(chat, i18n::client_moved(lang, &name, gname.as_deref()))
                .parse_mode(ParseMode::Html)
                .reply_markup(menu::main_menu(lang))
                .await?;
        }
        Action::GroupRegenAsk(id) => {
            let name = settings.group(id).map(|g| g.name).unwrap_or_default();
            let count = settings.group_client_count(id);
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::confirm_group_regen(lang, &name, count),
                menu::confirm_group_regen_menu(lang, id),
            )
            .await;
        }
        Action::GroupRegenRun(id) => {
            let clients = settings.group_client_names(id);
            let waiting = bot
                .send_message(chat, i18n::regen_all_running(lang))
                .await
                .ok();
            let (mut ok, mut failed) = (0usize, 0usize);
            for c in &clients {
                match client_refresh(&vpn, &settings, c).await {
                    Ok(_) => {
                        ok += 1;
                        settings.log_event(
                            now_epoch(),
                            EventKind::Regen,
                            Some(c),
                            Some(uid),
                            Some("group_regen"),
                        );
                    }
                    Err(e) => {
                        failed += 1;
                        tracing::error!(error = %e, client = %c, "regen в группе провалился");
                    }
                }
            }
            if let Some(m) = waiting {
                let _ = bot.delete_message(chat, m.id).await;
            }
            bot.send_message(chat, i18n::group_regen_done(lang, ok, failed))
                .reply_markup(menu::main_menu(lang))
                .parse_mode(ParseMode::Html)
                .await?;
        }
        Action::GroupScopeAsk => {
            let groups = settings.list_groups();
            edit_or_send(
                &bot,
                chat,
                msg_id,
                i18n::scope_title(lang),
                menu::group_scope_menu(lang, &groups),
            )
            .await;
        }
        Action::GroupScopeSet(scope) => {
            settings.set_owner_scope(uid, scope);
            render_clients_list(
                &bot,
                chat,
                msg_id,
                lang,
                &vpn,
                &settings,
                uid,
                0,
                scope,
                home_menu(&role, lang),
                role.is_owner(),
            )
            .await;
        }
        Action::Unknown => {
            bot.send_message(chat, unknown_action_text(lang)).await?;
        }
    }
    Ok(())
}

async fn pre_checkout_handler(
    bot: Bot,
    query: PreCheckoutQuery,
    settings: Arc<Store>,
) -> HandlerResult {
    let order = star_order_id(&query.invoice_payload).and_then(|id| settings.star_order(id));
    let valid = order.is_some_and(|order| {
        order.status == "pending"
            && order.user_id == query.from.id.0 as i64
            && query.currency == "XTR"
            && order.stars == i64::from(query.total_amount)
    });
    let mut request = bot.answer_pre_checkout_query(query.id, valid);
    if !valid {
        request = request.error_message(
            "Заказ устарел или его параметры изменились. Вернитесь в бот и создайте новый счёт.",
        );
    }
    request.await?;
    Ok(())
}

/// dptree-схема для `Dispatcher`. Зависимости (`Arc<Vpn>`, `Arc<Config>`,
/// `Arc<Store>`, `InMemStorage<State>`) регистрируются в `main` через
/// `dptree::deps![...]`.
pub fn schema() -> teloxide::dispatching::UpdateHandler<Box<dyn std::error::Error + Send + Sync>> {
    dptree::entry()
        .enter_dialogue::<Update, InMemStorage<State>, State>()
        .branch(Update::filter_message().endpoint(message_handler))
        .branch(Update::filter_pre_checkout_query().endpoint(pre_checkout_handler))
        .branch(Update::filter_callback_query().endpoint(callback_handler))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_navigation_cancels_promo_input() {
        for text in [
            "/start",
            "/start ref_42",
            "🏠 Кабинет",
            "🔑 Мои ключи",
            "➕ Купить ключ",
            "➕ Пополнить",
            "📖 Инструкция",
            "🆘 Поддержка",
            "🎟 Промокод",
        ] {
            assert!(is_customer_navigation(text), "{text}");
        }
        assert!(!is_customer_navigation("FRIEND25"));
    }

    #[test]
    fn group_for_new_client_recreate_preserves_binding() {
        // Recreate не трогает привязку: владелец не отвязывает клиента от его
        // группы, групповой админ не переносит клиента в свою текущую группу.
        let store = Store::open_in_memory();
        let a = store.create_group("a", 0).unwrap();
        let b = store.create_group("b", 0).unwrap();
        store.assign_client_group("alice", Some(a), 10);
        assert_eq!(
            group_for_new_client(&Role::Owner, &store, 1, true, "alice"),
            Some(Some(a))
        );
        store.add_group_admin(a, 42, 1, 0);
        store.add_group_admin(b, 42, 1, 0);
        store.set_current_group(42, b);
        let ga = Role::GroupAdmin(vec![a, b]);
        assert_eq!(
            group_for_new_client(&ga, &store, 42, true, "alice"),
            Some(Some(a))
        );
        // Клиент без группы у владельца остаётся без группы.
        assert_eq!(
            group_for_new_client(&Role::Owner, &store, 1, true, "nogroup"),
            Some(None)
        );
    }

    #[test]
    fn group_for_new_client_new_client_by_role() {
        // Новый клиент: групповому админу — его текущая группа (нет текущей →
        // None: нужен экран выбора), владельцу — без группы.
        let store = Store::open_in_memory();
        let a = store.create_group("a", 0).unwrap();
        let b = store.create_group("b", 0).unwrap();
        store.set_current_group(42, b);
        let ga = Role::GroupAdmin(vec![a, b]);
        assert_eq!(
            group_for_new_client(&ga, &store, 42, false, "bob"),
            Some(Some(b))
        );
        assert_eq!(
            group_for_new_client(&Role::Owner, &store, 1, false, "bob"),
            Some(None)
        );
        store.set_owner_scope(1, ListScope::Group(a));
        assert_eq!(
            group_for_new_client(&Role::Owner, &store, 1, false, "bob"),
            Some(Some(a))
        );
        // Сохранённая текущая группа отозвана → выбор группы.
        let ga_only_a = Role::GroupAdmin(vec![a, b]);
        store.set_current_group(43, 999);
        assert_eq!(
            group_for_new_client(&ga_only_a, &store, 43, false, "bob"),
            None
        );
    }

    #[test]
    fn add_rollback_only_for_new_client_full() {
        use crate::store::QuotaAssign;
        // Откат (удалить созданного клиента) — только когда НОВЫЙ клиент
        // проиграл гонку квоты. Recreate квоту не проверяет, Db — деградация
        // без отката (клиент остаётся без группы, как раньше).
        assert!(add_needs_quota_rollback(false, &QuotaAssign::Full));
        assert!(!add_needs_quota_rollback(true, &QuotaAssign::Full));
        assert!(!add_needs_quota_rollback(false, &QuotaAssign::Assigned));
        assert!(!add_needs_quota_rollback(false, &QuotaAssign::Db));
    }

    #[test]
    fn parses_all_actions() {
        assert_eq!(parse_callback("menu"), Action::Menu);
        assert_eq!(parse_callback("list"), Action::List);
        assert_eq!(parse_callback("add"), Action::Add);
        assert_eq!(parse_callback("stats"), Action::Stats);
        assert_eq!(parse_callback("page:3"), Action::Page(3));
        assert_eq!(
            parse_callback("client:alice"),
            Action::ShowClient("alice".into())
        );
        assert_eq!(
            parse_callback("conf:alice"),
            Action::SendConf("alice".into())
        );
        assert_eq!(
            parse_callback("del:alice"),
            Action::AskDelete("alice".into())
        );
        assert_eq!(
            parse_callback("delyes:alice"),
            Action::ConfirmDelete("alice".into())
        );
        assert_eq!(
            parse_callback("recreate:alice"),
            Action::Recreate("alice".into())
        );
        assert_eq!(parse_callback("exp:30d"), Action::Expiry("30d".into()));
        assert_eq!(
            parse_callback("exp:custom"),
            Action::Expiry("custom".into())
        );
        assert_eq!(parse_callback("settings"), Action::Settings);
        assert_eq!(parse_callback("lang:ru"), Action::Lang("ru".into()));
        assert_eq!(parse_callback("lang:en"), Action::Lang("en".into()));
        assert_eq!(parse_callback("set:lang:ru"), Action::SetLang("ru".into()));
        assert_eq!(parse_callback("set:lang:en"), Action::SetLang("en".into()));
        assert_eq!(parse_callback("set:psk:on"), Action::SetPsk(true));
        assert_eq!(parse_callback("set:psk:off"), Action::SetPsk(false));
        assert_eq!(parse_callback("set:slug:on"), Action::SetSlug(true));
        assert_eq!(parse_callback("set:slug:off"), Action::SetSlug(false));
        assert_eq!(parse_callback("add:psk:on"), Action::AddPsk(true));
        assert_eq!(parse_callback("add:psk:off"), Action::AddPsk(false));
        assert_eq!(parse_callback("add:server:42"), Action::AddServer(42));
        assert_eq!(parse_callback("backup"), Action::Backup);
        assert_eq!(parse_callback("bk:new"), Action::BackupNew);
        assert_eq!(parse_callback("bk:list"), Action::BackupList);
        assert_eq!(parse_callback("bk:restore_yes:2"), Action::RestoreYes(2));
        assert_eq!(parse_callback("bk:restore:2"), Action::Restore(2));
        assert_eq!(parse_callback("bk:dl:1"), Action::BackupDownload(1));
        assert_eq!(parse_callback("bk:card:0"), Action::BackupCard(0));
        assert_eq!(parse_callback("migration:local"), Action::LocalMigration);
        assert_eq!(
            parse_callback("migration:preflight"),
            Action::LocalMigrationPreflight
        );
        assert_eq!(
            parse_callback("migration:start"),
            Action::LocalMigrationStart
        );
        assert_eq!(
            parse_callback("migration:status"),
            Action::LocalMigrationStatus
        );
        assert_eq!(
            parse_callback("migration:rollback"),
            Action::LocalMigrationRollback
        );
        assert_eq!(parse_callback("check"), Action::Check);
        assert_eq!(parse_callback("garbage"), Action::Unknown);
    }

    #[test]
    fn parse_history_callback() {
        assert_eq!(
            parse_callback("history:alice"),
            Action::ClientHistory("alice".to_string())
        );
    }

    #[test]
    fn parse_callback_listfilter_variants() {
        use crate::vpn::model::ClientFilter;
        assert_eq!(
            parse_callback("listfilter:all"),
            Action::SetListFilter(ClientFilter::All)
        );
        assert_eq!(
            parse_callback("listfilter:online"),
            Action::SetListFilter(ClientFilter::Online)
        );
        assert_eq!(
            parse_callback("listfilter:offline"),
            Action::SetListFilter(ClientFilter::Offline)
        );
        assert_eq!(
            parse_callback("listfilter:never"),
            Action::SetListFilter(ClientFilter::Never)
        );
        // Неизвестное значение фильтра → Unknown (craftable callback guard).
        assert_eq!(parse_callback("listfilter:garbage"), Action::Unknown);
    }

    #[test]
    fn parse_callback_listfilter_does_not_collide_with_list() {
        // "list" — точный match (Action::List), "listfilter:..." — префикс.
        // Они не должны пересекаться.
        assert_eq!(parse_callback("list"), Action::List);
        assert!(matches!(
            parse_callback("listfilter:all"),
            Action::SetListFilter(_)
        ));
    }

    #[test]
    fn parse_callback_diagnose() {
        assert_eq!(parse_callback("diagnose"), Action::Diagnose);
    }

    #[test]
    fn parse_callback_regen_client() {
        assert_eq!(parse_callback("regen:alice"), Action::Regen("alice".into()));
    }

    #[test]
    fn parse_callback_regen_all_variants() {
        assert_eq!(parse_callback("regen_all"), Action::RegenAll);
        assert_eq!(parse_callback("regen_all_go"), Action::RegenAllRun(false));
        assert_eq!(
            parse_callback("regen_all_routes"),
            Action::RegenAllRun(true)
        );
        // "regen_all…" не должен съедаться префиксом "regen:" (там двоеточие).
        assert_eq!(parse_callback("regen:alice"), Action::Regen("alice".into()));
    }

    #[test]
    fn parses_bulk_and_artifact_actions() {
        assert_eq!(parse_callback("bulk:1"), Action::AddBulkRun(1));
        assert_eq!(parse_callback("bulk:10"), Action::AddBulkRun(10));
        assert_eq!(parse_callback("qr:alice"), Action::SendQr("alice".into()));
        assert_eq!(
            parse_callback("uri:alice"),
            Action::SendLink("alice".into())
        );
        assert_eq!(parse_callback("all:alice"), Action::SendAll("alice".into()));
        assert_eq!(parse_callback("set:conf:on"), Action::SetConf(true));
        assert_eq!(parse_callback("set:conf:off"), Action::SetConf(false));
        assert_eq!(parse_callback("set:qr:on"), Action::SetQr(true));
        assert_eq!(parse_callback("set:link:on"), Action::SetLink(true));
    }

    #[test]
    fn parse_callback_addbulk_keyword() {
        assert_eq!(parse_callback("addbulk"), Action::AddBulk);
    }

    #[test]
    fn parse_callback_bulk_expiry_and_psk() {
        assert_eq!(
            parse_callback("bulkexp:none"),
            Action::BulkExpiry("none".into())
        );
        assert_eq!(
            parse_callback("bulkexp:30d"),
            Action::BulkExpiry("30d".into())
        );
        assert_eq!(parse_callback("bulkadd:psk:on"), Action::AddBulkPsk(true));
        assert_eq!(parse_callback("bulkadd:psk:off"), Action::AddBulkPsk(false));
    }

    #[test]
    fn parse_callback_no_collision_uri_vs_other_prefixes() {
        // "uri:" не должен коллизировать с существующими префиксами
        assert_eq!(
            parse_callback("uri:alice"),
            Action::SendLink("alice".into())
        );
        // "all:" — тоже уникален
        assert_eq!(parse_callback("all:alice"), Action::SendAll("alice".into()));
    }

    #[test]
    fn parse_callback_modify_and_restart_and_repair() {
        assert_eq!(parse_callback("mod:alice"), Action::Modify("alice".into()));
        // modparam: должен парситься ДО mod: (длинный префикс), но mod:alice не
        // начинается с modparam:, так что отдельная проверка не нужна — проверяем сам modparam:.
        assert!(matches!(
            parse_callback("modparam:alice:keepalive"),
            Action::ModifyParam(_, _)
        ));
        assert_eq!(parse_callback("restart"), Action::Restart);
        assert_eq!(parse_callback("restart_go"), Action::RestartRun);
        assert_eq!(parse_callback("repair"), Action::RepairModule);
    }

    #[test]
    fn parse_callback_modparam_before_mod_prefix() {
        // modparam:... не должен триггерить mod: — но они разные по разделителю.
        // Проверка: modparam:x:y не парсится как Action::Modify.
        let r = parse_callback("modparam:x:keepalive");
        assert!(!matches!(r, Action::Modify(_)));
    }

    #[test]
    fn parse_callback_group_actions() {
        assert_eq!(parse_callback("groups"), Action::Groups);
        assert_eq!(parse_callback("g:new"), Action::GroupCreate);
        assert_eq!(parse_callback("g:card:5"), Action::GroupCard(5));
        assert_eq!(parse_callback("g:ren:5"), Action::GroupRenameAsk(5));
        assert_eq!(parse_callback("g:quota:5"), Action::GroupQuotaAsk(5));
        assert_eq!(parse_callback("g:adm:5"), Action::GroupAdmins(5));
        assert_eq!(
            parse_callback("g:admdel:5:42"),
            Action::GroupAdminRemove(5, 42)
        );
        assert_eq!(parse_callback("g:inv:5"), Action::GroupInvite(5));
        assert_eq!(parse_callback("g:invrev:5"), Action::GroupInviteRevoke(5));
        assert_eq!(parse_callback("g:admid:5"), Action::GroupAdminById(5));
        assert_eq!(parse_callback("g:del:5"), Action::GroupDeleteAsk(5));
        assert_eq!(
            parse_callback("g:deldetach:5"),
            Action::GroupDeleteDetach(5)
        );
        assert_eq!(parse_callback("g:delall:5"), Action::GroupDeleteAllAsk(5));
        assert_eq!(
            parse_callback("g:delallyes:5"),
            Action::GroupDeleteAllYes(5)
        );
        assert_eq!(parse_callback("g:regen:5"), Action::GroupRegenAsk(5));
        assert_eq!(parse_callback("g:regengo:5"), Action::GroupRegenRun(5));
        assert_eq!(parse_callback("g:sel:5"), Action::GroupSelect(5));
        assert_eq!(parse_callback("g:selmenu"), Action::GroupSelectMenu);
        assert_eq!(
            parse_callback("gmove:alice"),
            Action::MoveClientAsk("alice".into())
        );
        assert_eq!(
            parse_callback("gmoveto:none:alice"),
            Action::MoveClientTo(None, "alice".into())
        );
        assert_eq!(
            parse_callback("gmoveto:7:alice"),
            Action::MoveClientTo(Some(7), "alice".into())
        );
        // мусор → Unknown
        assert_eq!(parse_callback("g:card:x"), Action::Unknown);
        assert_eq!(parse_callback("gmoveto:xx:alice"), Action::Unknown);
    }

    #[test]
    fn parse_callback_group_scope() {
        use crate::store::ListScope;
        assert_eq!(parse_callback("gscope"), Action::GroupScopeAsk);
        assert_eq!(
            parse_callback("gscope:all"),
            Action::GroupScopeSet(ListScope::All)
        );
        assert_eq!(
            parse_callback("gscope:none"),
            Action::GroupScopeSet(ListScope::NoGroup)
        );
        assert_eq!(
            parse_callback("gscope:7"),
            Action::GroupScopeSet(ListScope::Group(7))
        );
        assert_eq!(parse_callback("gscope:x"), Action::Unknown);
    }

    #[test]
    fn empty_list_screen_no_clients_keeps_home_menu() {
        // Клиентов нет вообще — фильтровать нечего, остаётся домашнее меню.
        let home = menu::main_menu(Lang::Ru);
        let (text, kb) = empty_list_screen(
            Lang::Ru,
            0,
            crate::vpn::model::ClientFilter::All,
            true,
            home.clone(),
        );
        assert_eq!(text, i18n::clients_empty(Lang::Ru));
        assert_eq!(kb, home);
    }

    #[test]
    fn empty_list_screen_filtered_keeps_filter_controls() {
        // Тупик #20: клиенты есть, но липкий фильтр/скоуп дал пустую
        // выборку — экран обязан оставить кнопки смены фильтра и скоупа.
        let (text, kb) = empty_list_screen(
            Lang::Ru,
            3,
            crate::vpn::model::ClientFilter::All,
            true,
            menu::main_menu(Lang::Ru),
        );
        assert_eq!(text, i18n::clients_empty_filtered(Lang::Ru));
        let dbg = format!("{kb:?}");
        assert!(dbg.contains("\"gscope\""));
        assert!(dbg.contains("listfilter:all"));
    }

    #[test]
    fn truncate_for_message_respects_char_boundary() {
        // Трёхбайтовый символ: 3500 не кратно 3 → индекс попадает внутрь
        // символа, обрезка должна откатиться к границе, а не паниковать.
        let long = "€".repeat(1500); // 4500 байт
        let cut = truncate_for_message(long);
        assert!(cut.ends_with('…'));
        assert!(cut.len() <= 3504); // ≤3500 (до границы символа) + "\n…" (4 байта)
        let short = "ok".to_string();
        assert_eq!(truncate_for_message(short), "ok");
    }

    /// Замораживает контракт между слоем клавиатур (`menu`) и парсером
    /// callback-данных (`parse_callback`): каждая строка, которую эмитят
    /// клавиатуры, должна разбираться в осмысленный `Action`, а не в
    /// `Action::Unknown`. Это защищает от расхождения префиксов при
    /// будущих изменениях.
    #[test]
    fn all_menu_callback_data_parse_to_known_actions() {
        use crate::vpn::model::Client;
        use teloxide::types::{InlineKeyboardButtonKind, InlineKeyboardMarkup};

        fn all_callback_data(kb: &InlineKeyboardMarkup) -> Vec<String> {
            kb.inline_keyboard
                .iter()
                .flatten()
                .filter_map(|b| match &b.kind {
                    InlineKeyboardButtonKind::CallbackData(d) => Some(d.clone()),
                    _ => None,
                })
                .collect()
        }

        let sample_client = Client {
            name: "alice".into(),
            ip: String::new(),
            client_ipv6: String::new(),
            status: String::new(),
            status_code: "active".into(),
            rx: 0,
            tx: 0,
            last_handshake: None,
        };

        let sample_backup = crate::vpn::BackupFile {
            name: "awg_backup_x.tar.gz".into(),
            path: "x.tar.gz".into(),
            size: 1,
            mtime: 1,
        };

        fn sample_group() -> crate::store::GroupRow {
            crate::store::GroupRow {
                id: 1,
                name: "family".into(),
                max_clients: None,
                created_at: 0,
            }
        }

        let keyboards = vec![
            menu::main_menu(Lang::Ru),
            menu::admin_dashboard_menu(),
            menu::admin_keys_hub(),
            menu::admin_users_hub(),
            menu::admin_communication_hub(),
            menu::admin_system_hub(),
            menu::servers_menu(&[]),
            menu::server_card_menu(1),
            menu::remote_migration_menu(1),
            menu::vpn_service_menu(),
            menu::admin_create_menu(),
            menu::admin_roles_menu(),
            menu::admin_promos_menu(),
            menu::bulk_manage_menu(),
            menu::statistics_menu(),
            menu::admin_user_menu(42, false),
            menu::broadcast_audience_menu(),
            menu::support_filters_menu(&[]),
            menu::support_category_menu(),
            menu::support_ticket_menu(1),
            menu::support_rating_menu(1),
            menu::customer_key_menu("alice"),
            menu::customer_refresh_confirm_menu("alice"),
            menu::legacy_renew_menu("old_alice", 100_000),
            menu::legacy_renew_method_menu("old_alice"),
            menu::legacy_restore_menu(false),
            menu::legacy_restore_menu(true),
            menu::legacy_request_admin_menu(1),
            menu::legacy_admin_menu(&[]),
            menu::expiry_menu(Lang::Ru),
            menu::client_card(Lang::Ru, "alice", true),
            menu::confirm_delete(Lang::Ru, "bob"),
            menu::confirm_recreate(Lang::Ru, "alice"),
            menu::clients_list(
                Lang::Ru,
                &[sample_client],
                &[],
                0,
                0,
                8,
                crate::vpn::model::ClientFilter::All,
                true,
            ),
            menu::language_select(),
            menu::settings_menu(Lang::Ru, false, false, false, false, false),
            menu::settings_menu(Lang::Ru, true, true, true, true, true),
            menu::bulk_count_menu(Lang::Ru),
            menu::bulk_expiry_menu(Lang::Ru),
            menu::psk_step(Lang::Ru, false),
            menu::psk_step(Lang::Ru, true),
            menu::backup_menu(Lang::Ru),
            menu::backups_list(Lang::Ru, &[sample_backup]),
            menu::backup_card(Lang::Ru, 0),
            menu::confirm_restore(Lang::Ru, 0),
            menu::modify_param_menu(Lang::Ru, "alice"),
            menu::confirm_restart_menu(Lang::Ru),
            menu::groups_menu(Lang::Ru, &[(sample_group(), 2)]),
            menu::group_card_menu(Lang::Ru, 1, true),
            menu::group_card_menu(Lang::Ru, 1, false),
            menu::group_admins_menu(Lang::Ru, 1, &[42]),
            menu::group_delete_choice_menu(Lang::Ru, 1),
            menu::confirm_group_delete_clients_menu(Lang::Ru, 1),
            menu::confirm_group_regen_menu(Lang::Ru, 1),
            menu::group_select_menu(Lang::Ru, &[sample_group()]),
            menu::ga_main_menu(Lang::Ru, true),
            menu::move_client_menu(Lang::Ru, "alice", &[sample_group()]),
            menu::group_scope_menu(Lang::Ru, &[sample_group()]),
            menu::clients_empty_menu(Lang::Ru, crate::vpn::model::ClientFilter::All, true),
        ];

        for kb in &keyboards {
            for data in all_callback_data(kb) {
                assert_ne!(
                    parse_callback(&data),
                    Action::Unknown,
                    "callback data {data:?} did not parse to a known Action"
                );
            }
        }
    }

    /// По одному образцу каждого варианта Action — база для проверки полноты
    /// таблицы `authorize_table` ниже. Vec (в отличие от match в `authorize`)
    /// сам по себе не заставит компилятор напомнить про новый вариант,
    /// поэтому здесь есть отдельная компайл-страховка (см. ниже).
    fn coverage_samples() -> Vec<Action> {
        use Action::*;
        let samples = vec![
            AdminDashboard,
            AdminVpn,
            AdminServers,
            AdminKeys,
            AdminUsersHub,
            AdminCommunication,
            AdminSystem,
            ServerAdd,
            ServerCard(1),
            RemoteMigration(1),
            RemoteMigrationPreflight(1),
            RemoteMigrationStatus(1),
            RemoteMigrationTest(1),
            RemoteMigrationApprove(1),
            RemoteMigrationAsk(1),
            RemoteMigrationRun(1),
            RemoteMigrationRollback(1),
            ServerBilling,
            ServerBillingAsk(1),
            ServerPassportAsk(1),
            ServerEnroll(1),
            ServerEnrollRevoke(1),
            ServerSetDefault(1),
            ServerDeployAsk(1),
            ServerCheck(1),
            ServerDiagnose(1),
            ServerProvisioningProbe(1),
            ServerPanelConnect(1),
            ServerPanelSync(1),
            ServerPanelAudit(1),
            AdminCreate,
            AdminOwners,
            AdminFinance,
            AdminSupport,
            AdminBroadcast,
            AdminBroadcastTemplates,
            BroadcastAudience("all".into()),
            BroadcastRetry(1),
            AdminHelp,
            AdminSearch,
            AdminRoles,
            AdminRoleAction("add".into()),
            AdminBulk("menu".into()),
            AdminBulkConfirm,
            AdminUser(1),
            AdminUserKeys(1),
            AdminUserDeleteKeysAsk(1),
            AdminUserDeleteKeysConfirm(1),
            AdminUserPayments(1),
            AdminUserBalance(1),
            AdminUserDiscount(1),
            AdminUserNote(1),
            AdminUserBlock(1, true),
            StatsSection("vpn".into()),
            SupportFilter("open".into()),
            AdminPromos,
            AdminPromoAction("discount".into()),
            ClientNoteAsk("key".into()),
            LegacyRenew("key".into()),
            LegacyRenewMethod("key".into(), "manual".into()),
            LegacyRestore,
            LegacyRequestNew,
            LegacyRequestApprove(1),
            LegacyRequestReject(1),
            PromoInput,
            LegacyPriceAsk,
            Menu,
            List,
            Add,
            Stats,
            Page(0),
            ShowClient("s".into()),
            ClientHistory("s".into()),
            SendConf("s".into()),
            AskDelete("s".into()),
            ConfirmDelete("s".into()),
            Recreate("s".into()),
            Regen("s".into()),
            RegenAll,
            RegenAllRun(false),
            Expiry("none".into()),
            Lang("ru".into()),
            Settings,
            SetLang("ru".into()),
            SetPsk(false),
            SetSlug(false),
            AddPsk(false),
            AddServer(1),
            Backup,
            BackupNew,
            BackupList,
            BackupCard(0),
            BackupDownload(0),
            Restore(0),
            RestoreYes(0),
            Check,
            Diagnose,
            Modify("s".into()),
            ModifyParam("s".into(), crate::vpn::validate::ModifyParam::Dns),
            Restart,
            RestartRun,
            RepairModule,
            AddBulk,
            AddBulkRun(1),
            BulkExpiry("none".into()),
            AddBulkPsk(false),
            BulkServer(1),
            AcquiringUrlAsk,
            SendQr("s".into()),
            SendLink("s".into()),
            SendAll("s".into()),
            SetConf(false),
            SetQr(false),
            SetLink(false),
            SetListFilter(crate::vpn::model::ClientFilter::All),
            Groups,
            GroupCreate,
            GroupCard(0),
            GroupRenameAsk(0),
            GroupQuotaAsk(0),
            GroupAdmins(0),
            GroupAdminRemove(0, 0),
            GroupInvite(0),
            GroupInviteRevoke(0),
            GroupAdminById(0),
            GroupDeleteAsk(0),
            GroupDeleteDetach(0),
            GroupDeleteAllAsk(0),
            GroupDeleteAllYes(0),
            GroupRegenAsk(0),
            GroupRegenRun(0),
            GroupSelect(0),
            GroupSelectMenu,
            MoveClientAsk("s".into()),
            MoveClientTo(None, "s".into()),
            GroupScopeAsk,
            GroupScopeSet(crate::store::ListScope::All),
            Unknown,
        ];

        // Компайл-страховка: исчерпывающий match без wildcard по каждому
        // образцу. Добавили вариант в Action — этот match перестал
        // собираться, пока сюда не добавлен образец нового варианта (а
        // значит, и напоминание дописать для него строку в authorize_table).
        for sample in &samples {
            match sample {
                AdminDashboard => {}
                AdminVpn => {}
                AdminServers => {}
                AdminKeys => {}
                AdminUsersHub => {}
                AdminCommunication => {}
                AdminSystem => {}
                AdminUpdate => {}
                AdminUpdateRun => {}
                AdminUpdateStatus => {}
                AdminUpdateRollback => {}
                ServerAdd => {}
                ServerCard(_) => {}
                RemoteMigration(_) => {}
                RemoteMigrationPreflight(_) => {}
                RemoteMigrationStatus(_) => {}
                RemoteMigrationTest(_) => {}
                RemoteMigrationApprove(_) => {}
                RemoteMigrationAsk(_) => {}
                RemoteMigrationRun(_) => {}
                RemoteMigrationRollback(_) => {}
                ServerBilling => {}
                ServerBillingAsk(_) => {}
                ServerPassportAsk(_) => {}
                ServerEnroll(_) => {}
                ServerEnrollRevoke(_) => {}
                ServerSetDefault(_) => {}
                ServerDeployAsk(_) => {}
                ServerCheck(_) => {}
                ServerDiagnose(_) => {}
                ServerProvisioningProbe(_) => {}
                ServerPanelConnect(_) => {}
                ServerPanelSync(_) => {}
                ServerPanelAudit(_) => {}
                LocalMigration => {}
                LocalMigrationPreflight => {}
                LocalMigrationStart => {}
                LocalMigrationStatus => {}
                LocalMigrationRollback => {}
                AdminCreate => {}
                AdminOwners => {}
                AdminOwnersPage(_) => {}
                AdminFinance => {}
                AdminSupport => {}
                AdminBroadcast => {}
                AdminBroadcastTemplates => {}
                BroadcastAudience(_) => {}
                BroadcastRetry(_) => {}
                AdminHelp => {}
                AdminSearch => {}
                AdminRoles => {}
                AdminRoleAction(_) => {}
                AdminBulk(_) => {}
                AdminBulkConfirm => {}
                AdminUser(_) => {}
                AdminUserKeys(_) => {}
                AdminUserDeleteKeysAsk(_) => {}
                AdminUserDeleteKeysConfirm(_) => {}
                AdminUserPayments(_) => {}
                AdminUserBalance(_) => {}
                AdminUserDiscount(_) => {}
                AdminUserNote(_) => {}
                AdminUserBlock(_, _) => {}
                StatsSection(_) => {}
                SupportFilter(_) => {}
                AdminPromos => {}
                AdminCommerce => {}
                AdminPricesRub => {}
                AdminPricesStars => {}
                AdminReferral => {}
                AdminPromoAction(_) => {}
                ClientNoteAsk(_) => {}
                LegacyRenew(_) => {}
                LegacyRenewMethod(_, _) => {}
                LegacyRestore => {}
                LegacyRequestNew => {}
                LegacyRequestApprove(_) => {}
                LegacyRequestReject(_) => {}
                PromoInput => {}
                Guide(_) => {}
                LegacyPriceAsk => {}
                Menu => {}
                List => {}
                Add => {}
                Stats => {}
                Page(_) => {}
                ShowClient(_) => {}
                ClientHistory(_) => {}
                SendConf(_) => {}
                AskDelete(_) => {}
                ConfirmDelete(_) => {}
                Recreate(_) => {}
                Regen(_) => {}
                RegenAll => {}
                RegenAllRun(_) => {}
                Expiry(_) => {}
                Lang(_) => {}
                Settings => {}
                SetLang(_) => {}
                SetPsk(_) => {}
                SetSlug(_) => {}
                AddPsk(_) => {}
                AddServer(_) => {}
                Backup => {}
                BackupNew => {}
                BackupList => {}
                BackupCard(_) => {}
                BackupDownload(_) => {}
                Restore(_) => {}
                RestoreYes(_) => {}
                Check => {}
                Diagnose => {}
                Modify(_) => {}
                ModifyParam(_, _) => {}
                Restart => {}
                RestartRun => {}
                RepairModule => {}
                AddBulk => {}
                AddBulkRun(_) => {}
                BulkExpiry(_) => {}
                AddBulkPsk(_) => {}
                BulkServer(_) => {}
                SendQr(_) => {}
                SendLink(_) => {}
                SendAll(_) => {}
                SetConf(_) => {}
                SetQr(_) => {}
                SetLink(_) => {}
                SetListFilter(_) => {}
                Groups => {}
                GroupCreate => {}
                GroupCard(_) => {}
                GroupRenameAsk(_) => {}
                GroupQuotaAsk(_) => {}
                GroupAdmins(_) => {}
                GroupAdminRemove(_, _) => {}
                GroupInvite(_) => {}
                GroupInviteRevoke(_) => {}
                GroupAdminById(_) => {}
                GroupDeleteAsk(_) => {}
                GroupDeleteDetach(_) => {}
                GroupDeleteAllAsk(_) => {}
                GroupDeleteAllYes(_) => {}
                GroupRegenAsk(_) => {}
                GroupRegenRun(_) => {}
                GroupSelect(_) => {}
                GroupSelectMenu => {}
                MoveClientAsk(_) => {}
                MoveClientTo(_, _) => {}
                GroupScopeAsk => {}
                GroupScopeSet(_) => {}
                Buy => {}
                BuyServer(_) => {}
                BuyTerm(_) => {}
                BuyMethod(_, _) => {}
                BuyPaid(_) => {}
                MyKeys => {}
                Profile => {}
                Portal => {}
                Balance => {}
                PaymentApprove(_) => {}
                PaymentReject(_) => {}
                AssignOwnerAsk(_) => {}
                AdminExpiryAsk(_) => {}
                SetClientEnabled(_, _) => {}
                PaymentInstructionsAsk => {}
                AcquiringUrlAsk => {}
                CustomerKey(_) => {}
                CustomerMove(_) => {}
                CustomerMoveServer(_, _) => {}
                CustomerMoveConfirm(_) => {}
                CustomerMoveCancel(_) => {}
                CustomerRefresh(_) => {}
                CustomerRefreshRun(_) => {}
                Renew(_) => {}
                RenewTerm(_, _) => {}
                RenewMethod(_, _, _) => {}
                AutoRenew(_, _, _) => {}
                DeviceLabelAsk(_) => {}
                SupportTicket(_) => {}
                SupportNewCategory(_) => {}
                SupportTake(_) => {}
                SupportReply(_) => {}
                SupportClose(_) => {}
                SupportPriority(_, _) => {}
                SupportRate(_, _) => {}
                FinanceExport => {}
                Unknown => {}
            }
        }

        samples
    }

    /// Табличный тест гейта авторизации: каждая строка — один Action и
    /// ожидаемый доступ для owner/GA (снято с текущего поведения диспатча).
    /// Denied проверяется отдельно на каждой строке — защита в глубину.
    #[test]
    fn authorize_table() {
        use crate::store::ListScope;
        let store = Store::open_in_memory();
        let ga_group = store.create_group("a", 0).unwrap();
        let foreign = store.create_group("b", 0).unwrap();
        store.add_group_admin(ga_group, 42, 1, 0);
        store.assign_client_group("mine", Some(ga_group), 10);
        store.assign_client_group("theirs", Some(foreign), 10);
        // "free" — клиент без группы (строки в БД нет — client_group → None).
        let owner = Role::Owner;
        let ga = Role::GroupAdmin(vec![ga_group]);
        let denied = Role::Denied;

        // (action, разрешено owner, разрешено ga)
        let table: Vec<(Action, bool, bool)> = vec![
            // Общие.
            (Action::Menu, true, true),
            (Action::List, true, true),
            (Action::Add, true, true),
            (Action::Stats, true, true),
            (Action::Page(0), true, true),
            (Action::Expiry("1d".into()), true, true),
            (Action::AddPsk(true), true, true),
            (Action::AddServer(1), true, true),
            (Action::Lang("ru".into()), true, true),
            (
                Action::SetListFilter(crate::vpn::model::ClientFilter::All),
                true,
                true,
            ),
            (Action::Unknown, true, true),
            // Выбор группы: только GA, и только своя.
            (Action::GroupSelectMenu, false, true),
            (Action::GroupSelect(ga_group), false, true),
            (Action::GroupSelect(foreign), false, false),
            // Клиентские: владелец — все, GA — только свой скоуп.
            (Action::ShowClient("mine".into()), true, true),
            (Action::ShowClient("theirs".into()), true, false),
            (Action::ShowClient("free".into()), true, false),
            (Action::ClientHistory("mine".into()), true, true),
            (Action::ClientHistory("theirs".into()), true, false),
            (Action::SendConf("mine".into()), true, true),
            (Action::SendConf("theirs".into()), true, false),
            (Action::SendQr("mine".into()), true, true),
            (Action::SendQr("theirs".into()), true, false),
            (Action::SendLink("mine".into()), true, true),
            (Action::SendLink("theirs".into()), true, false),
            (Action::SendAll("mine".into()), true, true),
            (Action::SendAll("theirs".into()), true, false),
            (Action::AskDelete("mine".into()), true, true),
            (Action::AskDelete("theirs".into()), true, false),
            (Action::ConfirmDelete("mine".into()), true, true),
            (Action::ConfirmDelete("theirs".into()), true, false),
            (Action::Recreate("mine".into()), true, true),
            (Action::Recreate("theirs".into()), true, false),
            (Action::Regen("mine".into()), true, true),
            (Action::Regen("theirs".into()), true, false),
            // Owner-only.
            (Action::RegenAll, true, false),
            (Action::RegenAllRun(false), true, false),
            (Action::AddBulk, true, true),
            (Action::AddBulkRun(3), true, true),
            (Action::BulkExpiry("1d".into()), true, true),
            (Action::AddBulkPsk(true), true, true),
            (Action::BulkServer(1), true, true),
            (Action::Settings, true, false),
            (Action::SetLang("en".into()), true, false),
            (Action::SetPsk(true), true, false),
            (Action::SetSlug(true), true, false),
            (Action::SetConf(true), true, false),
            (Action::SetQr(true), true, false),
            (Action::SetLink(true), true, false),
            (Action::Modify("mine".into()), true, false),
            (
                Action::ModifyParam("mine".into(), crate::vpn::validate::ModifyParam::Dns),
                true,
                false,
            ),
            (Action::Restart, true, false),
            (Action::RestartRun, true, false),
            (Action::RepairModule, true, false),
            (Action::Backup, true, false),
            (Action::BackupNew, true, false),
            (Action::BackupList, true, false),
            (Action::BackupCard(0), true, false),
            (Action::BackupDownload(0), true, false),
            (Action::Restore(0), true, false),
            (Action::RestoreYes(0), true, false),
            (Action::Check, true, false),
            (Action::Diagnose, true, false),
            (Action::Groups, true, false),
            (Action::GroupCreate, true, false),
            (Action::GroupCard(ga_group), true, false),
            (Action::GroupRenameAsk(ga_group), true, false),
            (Action::GroupQuotaAsk(ga_group), true, false),
            (Action::GroupAdmins(ga_group), true, false),
            (Action::GroupAdminRemove(ga_group, 42), true, false),
            (Action::GroupInvite(ga_group), true, false),
            (Action::GroupInviteRevoke(ga_group), true, false),
            (Action::GroupAdminById(ga_group), true, false),
            (Action::GroupDeleteAsk(ga_group), true, false),
            (Action::GroupDeleteDetach(ga_group), true, false),
            (Action::GroupDeleteAllAsk(ga_group), true, false),
            (Action::GroupDeleteAllYes(ga_group), true, false),
            (Action::GroupRegenAsk(ga_group), true, false),
            (Action::GroupRegenRun(ga_group), true, false),
            (Action::MoveClientAsk("mine".into()), true, false),
            (
                Action::MoveClientTo(Some(ga_group), "mine".into()),
                true,
                false,
            ),
            (Action::GroupScopeAsk, true, false),
            (Action::GroupScopeSet(ListScope::All), true, false),
            // Пользовательские коммерческие действия доступны и владельцу,
            // и групповому администратору в общей таблице авторизации.
            (Action::Buy, true, true),
            (Action::BuyTerm(1), true, true),
            (Action::BuyMethod(1, "manual".into()), true, true),
            (Action::BuyPaid(1), true, true),
            (Action::MyKeys, true, true),
            (Action::Profile, true, true),
            (Action::Balance, true, true),
            (Action::CustomerKey("mine".into()), true, true),
            (Action::CustomerRefresh("mine".into()), true, true),
            (Action::CustomerRefreshRun("mine".into()), true, true),
            (Action::Renew("mine".into()), true, true),
            (Action::RenewTerm("mine".into(), 1), true, true),
            (
                Action::RenewMethod("mine".into(), 1, "manual".into()),
                true,
                true,
            ),
            (Action::LegacyRenew("mine".into()), true, true),
            (
                Action::LegacyRenewMethod("mine".into(), "manual".into()),
                true,
                true,
            ),
            (Action::LegacyRequestNew, true, true),
            (Action::PromoInput, true, true),
            (Action::AutoRenew("mine".into(), 1, true), true, true),
            (Action::DeviceLabelAsk("mine".into()), true, true),
            (Action::SupportRate(1, 5), true, true),
            // Остальные административные и операторские действия — owner-only.
            (Action::PaymentApprove(1), true, false),
            (Action::PaymentReject(1), true, false),
            (Action::AssignOwnerAsk("mine".into()), true, false),
            (Action::AdminExpiryAsk("mine".into()), true, false),
            (Action::SetClientEnabled("mine".into(), true), true, false),
            (Action::PaymentInstructionsAsk, true, false),
            (Action::SupportTicket(1), true, false),
            (Action::SupportNewCategory("general".into()), true, false),
            (Action::SupportTake(1), true, false),
            (Action::SupportReply(1), true, false),
            (Action::SupportClose(1), true, false),
            (Action::SupportPriority(1, "high".into()), true, false),
            (Action::FinanceExport, true, false),
            (Action::AdminDashboard, true, false),
            (Action::AdminVpn, true, false),
            (Action::AdminServers, true, false),
            (Action::AdminKeys, true, false),
            (Action::AdminUsersHub, true, false),
            (Action::AdminCommunication, true, false),
            (Action::AdminSystem, true, false),
            (Action::ServerAdd, true, false),
            (Action::ServerCard(1), true, false),
            (Action::RemoteMigration(1), true, false),
            (Action::RemoteMigrationPreflight(1), true, false),
            (Action::RemoteMigrationStatus(1), true, false),
            (Action::RemoteMigrationTest(1), true, false),
            (Action::RemoteMigrationApprove(1), true, false),
            (Action::RemoteMigrationAsk(1), true, false),
            (Action::RemoteMigrationRun(1), true, false),
            (Action::RemoteMigrationRollback(1), true, false),
            (Action::ServerBilling, true, false),
            (Action::ServerBillingAsk(1), true, false),
            (Action::ServerPassportAsk(1), true, false),
            (Action::ServerEnroll(1), true, false),
            (Action::ServerEnrollRevoke(1), true, false),
            (Action::ServerSetDefault(1), true, false),
            (Action::ServerDeployAsk(1), true, false),
            (Action::ServerCheck(1), true, false),
            (Action::ServerDiagnose(1), true, false),
            (Action::ServerProvisioningProbe(1), true, false),
            (Action::ServerPanelConnect(1), true, false),
            (Action::ServerPanelSync(1), true, false),
            (Action::ServerPanelAudit(1), true, false),
            (Action::LocalMigration, true, false),
            (Action::LocalMigrationPreflight, true, false),
            (Action::LocalMigrationStart, true, false),
            (Action::LocalMigrationStatus, true, false),
            (Action::LocalMigrationRollback, true, false),
            (Action::AdminCreate, true, false),
            (Action::AdminOwners, true, false),
            (Action::AdminFinance, true, false),
            (Action::AdminSupport, true, false),
            (Action::AdminBroadcast, true, false),
            (Action::AdminBroadcastTemplates, true, false),
            (Action::BroadcastAudience("all".into()), true, false),
            (Action::BroadcastRetry(1), true, false),
            (Action::AdminHelp, true, false),
            (Action::AdminSearch, true, false),
            (Action::AdminRoles, true, false),
            (Action::AdminRoleAction("add".into()), true, false),
            (Action::AdminBulk("menu".into()), true, false),
            (Action::AdminBulkConfirm, true, false),
            (Action::AdminUser(1), true, false),
            (Action::AdminUserKeys(1), true, false),
            (Action::AdminUserPayments(1), true, false),
            (Action::AdminUserBalance(1), true, false),
            (Action::AdminUserDiscount(1), true, false),
            (Action::AdminUserNote(1), true, false),
            (Action::AdminUserBlock(1, true), true, false),
            (Action::StatsSection("vpn".into()), true, false),
            (Action::SupportFilter("open".into()), true, false),
            (Action::AdminPromos, true, false),
            (Action::AdminPromoAction("legacy".into()), true, false),
            (Action::ClientNoteAsk("mine".into()), true, false),
            (Action::LegacyRestore, true, false),
            (Action::LegacyRequestApprove(1), true, false),
            (Action::LegacyRequestReject(1), true, false),
            (Action::LegacyPriceAsk, true, false),
            (Action::AcquiringUrlAsk, true, false),
        ];

        // Ассерт полноты: на каждый вариант Action (образцы из
        // coverage_samples) в таблице выше должна найтись хотя бы одна
        // строка — иначе новый вариант получит проверку доступа в
        // authorize(), но проскочит мимо теста молча.
        let table_discriminants: std::collections::HashSet<_> = table
            .iter()
            .map(|(action, _, _)| std::mem::discriminant(action))
            .collect();
        for sample in coverage_samples() {
            assert!(
                table_discriminants.contains(&std::mem::discriminant(&sample)),
                "authorize_table не содержит строки для варианта {sample:?} — допишите строку в table"
            );
        }

        for (action, owner_ok, ga_ok) in &table {
            assert_eq!(
                authorize(action, &owner, &store),
                *owner_ok,
                "owner: {action:?}"
            );
            assert_eq!(authorize(action, &ga, &store), *ga_ok, "ga: {action:?}");
            // Denied не проходит НИЧЕГО — защита в глубину (обычно отсекается
            // раньше, на входе в handle_callback).
            assert!(!authorize(action, &denied, &store), "denied: {action:?}");
        }
    }
}
