[Unit]
Description={{PROJECT}} backend ({{STACK}})
After=network.target

[Service]
Type=simple
User={{USER}}
WorkingDirectory={{BACKEND_DIR}}
Environment=PORT={{PORT}}
Environment=NODE_ENV=production
Environment=DATABASE_PATH={{DB_DIR}}
Environment=UPLOADS_PATH={{UPLOADS_DIR}}
ExecStart={{EXEC_START}}
Restart=always
RestartSec=3
StandardOutput=journal
StandardError=journal

[Install]
WantedBy=multi-user.target