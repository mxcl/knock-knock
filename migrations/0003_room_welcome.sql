ALTER TABLE rooms ADD COLUMN welcome_message TEXT;
ALTER TABLE rooms ADD COLUMN welcome_by INTEGER REFERENCES users(id);
