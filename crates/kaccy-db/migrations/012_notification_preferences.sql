-- Migration: Notification Preferences Table
-- Description: Create a table to store user notification preferences
-- Created: 2026-01-18

-- Create notification_preferences table
CREATE TABLE IF NOT EXISTS notification_preferences (
    user_id UUID PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,

    -- Email preferences
    email_enabled BOOLEAN NOT NULL DEFAULT true,
    email_order_updates BOOLEAN NOT NULL DEFAULT true,
    email_trade_notifications BOOLEAN NOT NULL DEFAULT true,
    email_commitment_updates BOOLEAN NOT NULL DEFAULT true,
    email_kyc_updates BOOLEAN NOT NULL DEFAULT true,
    email_marketing BOOLEAN NOT NULL DEFAULT false,

    -- SMS preferences
    sms_enabled BOOLEAN NOT NULL DEFAULT false,
    sms_order_updates BOOLEAN NOT NULL DEFAULT false,
    sms_trade_notifications BOOLEAN NOT NULL DEFAULT false,
    sms_security_alerts BOOLEAN NOT NULL DEFAULT true,

    -- Push notification preferences
    push_enabled BOOLEAN NOT NULL DEFAULT true,
    push_order_updates BOOLEAN NOT NULL DEFAULT true,
    push_trade_notifications BOOLEAN NOT NULL DEFAULT true,
    push_price_alerts BOOLEAN NOT NULL DEFAULT false,

    -- In-app notification preferences
    inapp_enabled BOOLEAN NOT NULL DEFAULT true,

    -- Metadata
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- Create index on user_id (primary key already creates one, but explicit for clarity)
CREATE INDEX IF NOT EXISTS idx_notification_preferences_user_id ON notification_preferences(user_id);

-- Add comment on table
COMMENT ON TABLE notification_preferences IS 'Stores user notification preferences for email, SMS, push, and in-app notifications';
COMMENT ON COLUMN notification_preferences.user_id IS 'Reference to the user';
COMMENT ON COLUMN notification_preferences.email_enabled IS 'Master toggle for all email notifications';
COMMENT ON COLUMN notification_preferences.email_order_updates IS 'Receive order status update emails';
COMMENT ON COLUMN notification_preferences.email_trade_notifications IS 'Receive trade execution emails';
COMMENT ON COLUMN notification_preferences.email_commitment_updates IS 'Receive commitment status emails';
COMMENT ON COLUMN notification_preferences.email_kyc_updates IS 'Receive KYC verification emails';
COMMENT ON COLUMN notification_preferences.email_marketing IS 'Receive marketing and promotional emails';
COMMENT ON COLUMN notification_preferences.sms_enabled IS 'Master toggle for all SMS notifications';
COMMENT ON COLUMN notification_preferences.sms_order_updates IS 'Receive order status update SMS';
COMMENT ON COLUMN notification_preferences.sms_trade_notifications IS 'Receive trade execution SMS';
COMMENT ON COLUMN notification_preferences.sms_security_alerts IS 'Receive security alert SMS';
COMMENT ON COLUMN notification_preferences.push_enabled IS 'Master toggle for all push notifications';
COMMENT ON COLUMN notification_preferences.push_order_updates IS 'Receive order status push notifications';
COMMENT ON COLUMN notification_preferences.push_trade_notifications IS 'Receive trade execution push notifications';
COMMENT ON COLUMN notification_preferences.push_price_alerts IS 'Receive price alert push notifications';
COMMENT ON COLUMN notification_preferences.inapp_enabled IS 'Master toggle for all in-app notifications';
