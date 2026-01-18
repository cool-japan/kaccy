//! Notification Preferences Example
//!
//! This example demonstrates how to use the NotificationPreferencesRepository for:
//! - Creating and managing user notification preferences
//! - Updating individual preference types (email, SMS, push, in-app)
//! - Checking if notifications should be sent
//! - Batch operations for notification management
//! - Marketing email list management
//!
//! Note: This example uses runtime queries and does not require DATABASE_URL to compile.

use kaccy_db::{
    create_pool, CreateNotificationPreferences, EmailNotificationType,
    NotificationPreferencesRepository, PushNotificationType, SmsNotificationType,
    UpdateNotificationPreferences,
};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Notification Preferences Example ===\n");

    // In a real application, this would come from environment variables
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://localhost/kaccy".to_string());

    println!("Connecting to database...");
    let pool = create_pool(&database_url).await?;
    println!("✓ Connected to database\n");

    // Initialize repository
    let prefs_repo = NotificationPreferencesRepository::new(pool.clone());

    // Example user ID (in production, this would come from your user system)
    let user_id = Uuid::new_v4();

    // =========================================================================
    // 1. Creating Default Notification Preferences
    // =========================================================================
    println!("1. Creating Default Notification Preferences");
    println!("   -----------------------------------------");

    // Get or create default preferences for a new user
    let prefs = prefs_repo.get_or_create_default(user_id).await?;
    println!("   ✓ Created default preferences for user {}", user_id);
    println!("     - Email enabled: {}", prefs.email_enabled);
    println!("     - SMS enabled: {}", prefs.sms_enabled);
    println!("     - Push enabled: {}", prefs.push_enabled);
    println!("     - In-app enabled: {}", prefs.inapp_enabled);
    println!();

    // =========================================================================
    // 2. Creating Custom Notification Preferences
    // =========================================================================
    println!("2. Creating Custom Notification Preferences");
    println!("   ----------------------------------------");

    let user_id_2 = Uuid::new_v4();

    // Create preferences with custom settings
    let custom_prefs = prefs_repo
        .create(&CreateNotificationPreferences {
            user_id: user_id_2,
            email_enabled: Some(true),
            email_order_updates: Some(true),
            email_trade_notifications: Some(true),
            email_commitment_updates: Some(false), // User doesn't want commitment emails
            email_kyc_updates: Some(true),
            email_marketing: Some(false), // User opted out of marketing
            sms_enabled: Some(true),
            sms_order_updates: Some(false),
            sms_trade_notifications: Some(false),
            sms_security_alerts: Some(true), // Only security alerts via SMS
            push_enabled: Some(true),
            push_order_updates: Some(true),
            push_trade_notifications: Some(true),
            push_price_alerts: Some(true), // User wants price alerts
            inapp_enabled: Some(true),
        })
        .await?;

    println!("   ✓ Created custom preferences for user {}", user_id_2);
    println!("     - Email marketing: {}", custom_prefs.email_marketing);
    println!(
        "     - SMS security alerts only: {}",
        custom_prefs.sms_security_alerts
    );
    println!(
        "     - Push price alerts: {}",
        custom_prefs.push_price_alerts
    );
    println!();

    // =========================================================================
    // 3. Updating Notification Preferences
    // =========================================================================
    println!("3. Updating Notification Preferences");
    println!("   ---------------------------------");

    // Update specific preferences
    let updated_prefs = prefs_repo
        .update(
            user_id,
            &UpdateNotificationPreferences {
                email_marketing: Some(true),   // User opted in to marketing
                push_price_alerts: Some(true), // Enable price alerts
                ..Default::default()
            },
        )
        .await?;

    println!("   ✓ Updated preferences for user {}", user_id);
    println!("     - Email marketing: {}", updated_prefs.email_marketing);
    println!(
        "     - Push price alerts: {}",
        updated_prefs.push_price_alerts
    );
    println!();

    // =========================================================================
    // 4. Batch Updating Email Preferences
    // =========================================================================
    println!("4. Batch Updating Email Preferences");
    println!("   --------------------------------");

    let email_prefs = prefs_repo
        .update_email_preferences(
            user_id,
            Some(true),  // Email enabled
            Some(true),  // Order updates
            Some(true),  // Trade notifications
            Some(true),  // Commitment updates
            Some(true),  // KYC updates
            Some(false), // No marketing
        )
        .await?;

    println!("   ✓ Updated all email preferences for user {}", user_id);
    println!("     - Order updates: {}", email_prefs.email_order_updates);
    println!(
        "     - Trade notifications: {}",
        email_prefs.email_trade_notifications
    );
    println!(
        "     - Commitment updates: {}",
        email_prefs.email_commitment_updates
    );
    println!("     - Marketing: {}", email_prefs.email_marketing);
    println!();

    // =========================================================================
    // 5. Batch Updating SMS Preferences
    // =========================================================================
    println!("5. Batch Updating SMS Preferences");
    println!("   ------------------------------");

    let sms_prefs = prefs_repo
        .update_sms_preferences(
            user_id,
            Some(true),  // SMS enabled
            Some(false), // No order updates
            Some(false), // No trade notifications
            Some(true),  // Security alerts only
        )
        .await?;

    println!("   ✓ Updated all SMS preferences for user {}", user_id);
    println!("     - SMS enabled: {}", sms_prefs.sms_enabled);
    println!("     - Order updates: {}", sms_prefs.sms_order_updates);
    println!(
        "     - Trade notifications: {}",
        sms_prefs.sms_trade_notifications
    );
    println!("     - Security alerts: {}", sms_prefs.sms_security_alerts);
    println!();

    // =========================================================================
    // 6. Batch Updating Push Notification Preferences
    // =========================================================================
    println!("6. Batch Updating Push Notification Preferences");
    println!("   -------------------------------------------");

    let push_prefs = prefs_repo
        .update_push_preferences(
            user_id,
            Some(true), // Push enabled
            Some(true), // Order updates
            Some(true), // Trade notifications
            Some(true), // Price alerts
        )
        .await?;

    println!("   ✓ Updated all push preferences for user {}", user_id);
    println!("     - Push enabled: {}", push_prefs.push_enabled);
    println!("     - Order updates: {}", push_prefs.push_order_updates);
    println!(
        "     - Trade notifications: {}",
        push_prefs.push_trade_notifications
    );
    println!("     - Price alerts: {}", push_prefs.push_price_alerts);
    println!();

    // =========================================================================
    // 7. Checking if Notifications Should Be Sent
    // =========================================================================
    println!("7. Checking if Notifications Should Be Sent");
    println!("   ----------------------------------------");

    // Check email notification permissions
    let should_send_order_email = prefs_repo
        .should_send_email(user_id, EmailNotificationType::OrderUpdate)
        .await?;
    let should_send_marketing_email = prefs_repo
        .should_send_email(user_id, EmailNotificationType::Marketing)
        .await?;

    println!("   ✓ Email notification checks:");
    println!(
        "     - Should send order update email: {}",
        should_send_order_email
    );
    println!(
        "     - Should send marketing email: {}",
        should_send_marketing_email
    );

    // Check SMS notification permissions
    let should_send_security_sms = prefs_repo
        .should_send_sms(user_id, SmsNotificationType::SecurityAlert)
        .await?;
    let should_send_order_sms = prefs_repo
        .should_send_sms(user_id, SmsNotificationType::OrderUpdate)
        .await?;

    println!("   ✓ SMS notification checks:");
    println!(
        "     - Should send security alert SMS: {}",
        should_send_security_sms
    );
    println!(
        "     - Should send order update SMS: {}",
        should_send_order_sms
    );

    // Check push notification permissions
    let should_send_price_push = prefs_repo
        .should_send_push(user_id, PushNotificationType::PriceAlert)
        .await?;
    let should_send_trade_push = prefs_repo
        .should_send_push(user_id, PushNotificationType::TradeNotification)
        .await?;

    println!("   ✓ Push notification checks:");
    println!(
        "     - Should send price alert push: {}",
        should_send_price_push
    );
    println!(
        "     - Should send trade notification push: {}",
        should_send_trade_push
    );

    // Check in-app notification permissions
    let should_send_inapp = prefs_repo.should_send_inapp(user_id).await?;
    println!("   ✓ In-app notification check:");
    println!(
        "     - Should send in-app notifications: {}",
        should_send_inapp
    );
    println!();

    // =========================================================================
    // 8. Disabling All Notifications (Account Suspension)
    // =========================================================================
    println!("8. Disabling All Notifications (Account Suspension)");
    println!("   -----------------------------------------------");

    let disabled_prefs = prefs_repo.disable_all(user_id).await?;
    println!("   ✓ Disabled all notifications for user {}", user_id);
    println!("     - Email enabled: {}", disabled_prefs.email_enabled);
    println!("     - SMS enabled: {}", disabled_prefs.sms_enabled);
    println!("     - Push enabled: {}", disabled_prefs.push_enabled);
    println!("     - In-app enabled: {}", disabled_prefs.inapp_enabled);
    println!();

    // =========================================================================
    // 9. Re-enabling Default Notifications (Account Reactivation)
    // =========================================================================
    println!("9. Re-enabling Default Notifications (Account Reactivation)");
    println!("   --------------------------------------------------------");

    let enabled_prefs = prefs_repo.enable_defaults(user_id).await?;
    println!("   ✓ Enabled default notifications for user {}", user_id);
    println!("     - Email enabled: {}", enabled_prefs.email_enabled);
    println!("     - SMS enabled: {}", enabled_prefs.sms_enabled);
    println!("     - Push enabled: {}", enabled_prefs.push_enabled);
    println!("     - In-app enabled: {}", enabled_prefs.inapp_enabled);
    println!();

    // =========================================================================
    // 10. Marketing Email List Management
    // =========================================================================
    println!("10. Marketing Email List Management");
    println!("    -------------------------------");

    // Count users who opted in to marketing emails
    let marketing_count = prefs_repo.count_marketing_enabled().await?;
    println!(
        "   ✓ Total users opted in to marketing: {}",
        marketing_count
    );

    // Get users for a marketing campaign (paginated)
    let marketing_users = prefs_repo
        .get_marketing_enabled_users(10, 0) // First 10 users
        .await?;
    println!(
        "   ✓ Retrieved {} marketing-enabled users (first page)",
        marketing_users.len()
    );
    println!();

    // =========================================================================
    // 11. Batch Creating Default Preferences for New Users
    // =========================================================================
    println!("11. Batch Creating Default Preferences for New Users");
    println!("    -----------------------------------------------");

    // Simulate onboarding multiple users at once
    let new_user_ids = vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()];

    let mut transaction = pool.begin().await?;
    let created_count = prefs_repo
        .batch_create_defaults(&mut *transaction, &new_user_ids)
        .await?;
    transaction.commit().await?;

    println!(
        "   ✓ Created default preferences for {} new users",
        created_count
    );
    println!();

    // =========================================================================
    // 12. Retrieving Notification Preferences
    // =========================================================================
    println!("12. Retrieving Notification Preferences");
    println!("    -----------------------------------");

    // Get preferences for a user
    if let Some(retrieved_prefs) = prefs_repo.get(user_id).await? {
        println!("   ✓ Retrieved preferences for user {}", user_id);
        println!("     - Created at: {}", retrieved_prefs.created_at);
        println!("     - Updated at: {}", retrieved_prefs.updated_at);
        println!("     - Email enabled: {}", retrieved_prefs.email_enabled);
        println!("     - SMS enabled: {}", retrieved_prefs.sms_enabled);
        println!("     - Push enabled: {}", retrieved_prefs.push_enabled);
    }
    println!();

    // =========================================================================
    // 13. Deleting Notification Preferences (Account Deletion)
    // =========================================================================
    println!("13. Deleting Notification Preferences (Account Deletion)");
    println!("    ---------------------------------------------------");

    let deleted = prefs_repo.delete(user_id_2).await?;
    if deleted {
        println!("   ✓ Deleted preferences for user {}", user_id_2);
    } else {
        println!("   ⚠ Preferences not found for user {}", user_id_2);
    }
    println!();

    // =========================================================================
    // Complete!
    // =========================================================================
    println!("=== Example Complete ===");
    println!();
    println!("This example demonstrated:");
    println!("  ✓ Creating default and custom notification preferences");
    println!("  ✓ Updating individual and batch preferences");
    println!("  ✓ Checking notification permissions before sending");
    println!("  ✓ Disabling/enabling all notifications");
    println!("  ✓ Marketing email list management");
    println!("  ✓ Batch operations for efficiency");
    println!("  ✓ Account lifecycle management");

    Ok(())
}
