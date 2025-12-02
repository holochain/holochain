-- Conductor database schema
-- Stores conductor state, installed apps, and related metadata

-- Conductor metadata (singleton table)
CREATE TABLE IF NOT EXISTS Conductor (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    tag TEXT NOT NULL
) STRICT;

-- Installed applications
CREATE TABLE IF NOT EXISTS InstalledApp (
    app_id TEXT PRIMARY KEY,
    agent_pub_key BLOB NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('enabled', 'disabled', 'awaiting_memproofs')),
    disabled_reason TEXT,
    manifest_blob BLOB NOT NULL,
    role_assignments_blob BLOB NOT NULL,
    installed_at INTEGER NOT NULL
) STRICT;

-- App role assignments (one row per role per app)
CREATE TABLE IF NOT EXISTS AppRole (
    app_id TEXT NOT NULL,
    role_name TEXT NOT NULL,
    dna_hash BLOB NOT NULL,
    is_clone_limit_enabled INTEGER NOT NULL DEFAULT 0,
    clone_limit INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (app_id, role_name),
    FOREIGN KEY (app_id) REFERENCES InstalledApp(app_id) ON DELETE CASCADE
) STRICT;

-- Clone cells (dynamically created cells for a role)
CREATE TABLE IF NOT EXISTS CloneCell (
    app_id TEXT NOT NULL,
    role_name TEXT NOT NULL,
    clone_id TEXT NOT NULL,
    dna_hash BLOB NOT NULL,
    is_enabled INTEGER NOT NULL DEFAULT 1,
    PRIMARY KEY (app_id, role_name, clone_id),
    FOREIGN KEY (app_id, role_name) REFERENCES AppRole(app_id, role_name) ON DELETE CASCADE
) STRICT;

-- App interfaces (websocket connections for apps)
CREATE TABLE IF NOT EXISTS AppInterface (
    port INTEGER NOT NULL,
    id TEXT,
    driver_type TEXT NOT NULL CHECK (driver_type = 'websocket'),
    websocket_port INTEGER,
    danger_bind_addr TEXT,
    allowed_origins_blob BLOB,
    installed_app_id TEXT,
    PRIMARY KEY (port, id),
    FOREIGN KEY (installed_app_id) REFERENCES InstalledApp(app_id) ON DELETE SET NULL
) STRICT;

-- Signal subscriptions per app per interface
CREATE TABLE IF NOT EXISTS SignalSubscription (
    interface_port INTEGER NOT NULL,
    interface_id TEXT,
    app_id TEXT NOT NULL,
    filters_blob BLOB,
    FOREIGN KEY (interface_port, interface_id) REFERENCES AppInterface(port, id) ON DELETE CASCADE,
    FOREIGN KEY (app_id) REFERENCES InstalledApp(app_id) ON DELETE CASCADE
) STRICT;

-- Indexes for common queries
CREATE INDEX IF NOT EXISTS idx_installed_app_status ON InstalledApp(status);
CREATE INDEX IF NOT EXISTS idx_app_role_dna_hash ON AppRole(dna_hash);
CREATE INDEX IF NOT EXISTS idx_clone_cell_enabled ON CloneCell(is_enabled);
CREATE INDEX IF NOT EXISTS idx_app_interface_app_id ON AppInterface(installed_app_id);
