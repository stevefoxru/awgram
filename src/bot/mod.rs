pub mod handlers;
pub mod menu;
pub mod render;

use crate::vpn::validate::ModifyParam;

#[derive(Clone, Default)]
pub enum State {
    #[default]
    Idle,
    AwaitingName,
    AwaitingExpiry {
        name: String,
        recreate: bool,
    },
    AwaitingCustomExpiry {
        name: String,
        recreate: bool,
    },
    AwaitingPsk {
        name: String,
        expires: Option<String>,
        recreate: bool,
    },
    AwaitingAddServer {
        name: String,
        expires: Option<String>,
        psk: bool,
        group: Option<i64>,
    },
    AwaitingModifyParam {
        name: String,
    },
    AwaitingModifyValue {
        name: String,
        param: ModifyParam,
    },
    // --- массовая генерация (отдельные state, не перегружают одиночные) ---
    AwaitingBulkPrefix,
    AwaitingBulkCount {
        prefix: String,
    },
    AwaitingBulkExpiry {
        prefix: String,
        count: usize,
    },
    AwaitingBulkCustomExpiry {
        prefix: String,
        count: usize,
    },
    AwaitingBulkPsk {
        prefix: String,
        count: usize,
        expires: Option<String>,
    },
    AwaitingBulkServer {
        prefix: String,
        count: usize,
        expires: Option<String>,
        psk: bool,
    },
    // --- группы (#20): диалоги владельца ---
    AwaitingGroupName,
    AwaitingGroupRename {
        id: i64,
    },
    AwaitingGroupQuota {
        id: i64,
    },
    AwaitingGroupAdminId {
        id: i64,
    },
    AwaitingPaymentProof {
        id: i64,
    },
    AwaitingPaymentReject {
        id: i64,
    },
    AwaitingClientOwner {
        name: String,
    },
    AwaitingAdminExpiry {
        name: String,
    },
    AwaitingDeviceLabel {
        name: String,
    },
    AwaitingKeyTransfer {
        name: String,
    },
    AwaitingTopupAmount,
    AwaitingPaymentInstructions,
    AwaitingAcquiringUrl,
    AwaitingPortalDomain,
    AwaitingTariffPricesRub,
    AwaitingTariffPricesStars,
    AwaitingReferralPercent,
    AwaitingPartnerDetails,
    AwaitingPartnerToken {
        partner_id: i64,
    },
    AwaitingSupportMessage {
        category: String,
        ticket_id: Option<i64>,
    },
    AwaitingSupportReply {
        ticket_id: i64,
        user_id: i64,
    },
    AwaitingBroadcast {
        audience: String,
    },
    AwaitingBroadcastConfirm {
        source_chat_id: i64,
        source_message_id: i32,
        audience: String,
    },
    AwaitingAdminSearch,
    AwaitingStaffRole {
        operation: String,
    },
    AwaitingBulkManage {
        operation: String,
    },
    AwaitingBulkConfirm {
        operation: String,
        prefix: String,
        names: Vec<String>,
        seconds: Option<i64>,
    },
    AwaitingUserBalance {
        user_id: i64,
    },
    AwaitingUserDiscount {
        user_id: i64,
    },
    AwaitingUserNote {
        user_id: i64,
    },
    AwaitingClientNote {
        name: String,
    },
    AwaitingPromoCode {
        kind: String,
    },
    AwaitingCustomerPromo,
    AwaitingLegacyRequest,
    AwaitingLegacyReject {
        id: i64,
    },
    AwaitingLegacyPrice,
    AwaitingServerAdd,
    AwaitingServerWizardName,
    AwaitingServerWizardAddress {
        name: String,
    },
    AwaitingServerWizardDetails {
        name: String,
        public_ip: String,
    },
    AwaitingServerBilling {
        server_id: i64,
    },
    AwaitingServerPassport {
        server_id: i64,
    },
    AwaitingServerField {
        server_id: i64,
        field: String,
    },
    AwaitingServerDeployCredentials {
        server_id: i64,
    },
    AwaitingPanelCredentials {
        server_id: i64,
    },
    AwaitingMirrorToken,
    AwaitingLocalMigrationConfirm {
        operation: String,
    },
}

#[cfg(test)]
mod tests {
    use super::State;

    #[test]
    fn bulk_state_variants_exist() {
        let _ = State::AwaitingBulkPrefix;
        let _ = State::AwaitingBulkCount {
            prefix: "user".into(),
        };
        let _ = State::AwaitingBulkExpiry {
            prefix: "user".into(),
            count: 10,
        };
        let _ = State::AwaitingBulkCustomExpiry {
            prefix: "user".into(),
            count: 10,
        };
        let _ = State::AwaitingBulkPsk {
            prefix: "user".into(),
            count: 10,
            expires: None,
        };
    }
}
