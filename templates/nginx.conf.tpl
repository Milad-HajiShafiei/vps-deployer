# {{PROJECT}} — nginx vhost
# Edit freely, then reinstall via "Deploy (my edits)" in the TUI.
server {
    listen 80;
    listen [::]:80;
    server_name {{DOMAIN}};

    root {{DIST}};
    index index.html;
    client_max_body_size {{MAX_BODY}};

    location {{PREFIX}} {
        proxy_pass http://127.0.0.1:{{PORT}};
        proxy_http_version 1.1;
{{WEBSOCKET_BLOCK}}
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }

    location / {
        try_files $uri $uri/ /index.html;
    }
}