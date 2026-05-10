-- Migration: add suggestion_code column to issues table
-- This column stores the suggested code fix from review bot

ALTER TABLE issues
ADD COLUMN IF NOT EXISTS suggestion_code TEXT;
