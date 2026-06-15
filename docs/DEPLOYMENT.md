# Deployment

This guide covers deploying axum-kickoff to production environments.

## Overview

axum-kickoff is designed for easy deployment with minimal dependencies. It can be deployed using:

- Docker containers
- Systemd services
- Cloud platforms (AWS, GCP, DigitalOcean, etc.)
- Platform-as-a-Service (Heroku, Railway, etc.)

## Pre-Deployment Checklist

Before deploying to production:

- [ ] Generate secure session key (64+ bytes)
- [ ] Configure PostgreSQL database (not SQLite)
- [ ] Set up GitHub OAuth production app
- [ ] Configure S3 or equivalent storage
- [ ] Enable HTTPS with valid SSL certificate
- [ ] Configure environment variables
- [ ] Set appropriate rate limits
- [ ] Enable security headers (HSTS, CSP)
- [ ] Configure logging and monitoring
- [ ] Set up database backups
- [ ] Review CORS origins

## Environment Variables

Ensure all required environment variables are set in production:

```bash
# Database
DATABASE_URL=postgresql://user:password@host:5432/dbname

# Session
SESSION_KEY=<generate-secure-64-byte-key>

# Server
PORT=3000
DOMAIN_NAME=example.com
SERVER_IP=0.0.0.0

# GitHub OAuth
GITHUB_CLIENT_ID=production_client_id
GITHUB_CLIENT_SECRET=production_client_secret
GITHUB_REDIRECT_URI=https://example.com/auth/github/callback

# CORS
WEB_ALLOWED_ORIGINS=https://example.com

# Storage
STORAGE_BACKEND=s3
STORAGE_S3_BUCKET=production-bucket
STORAGE_S3_REGION=us-east-1
STORAGE_S3_ACCESS_KEY=access_key
STORAGE_S3_SECRET_KEY=secret_key

# Security
SECURITY_HSTS_ENABLED=true
SECURITY_HSTS_MAX_AGE=31536000
SECURITY_HSTS_INCLUDE_SUBDOMAINS=true
SECURITY_HSTS_PRELOAD=true
SECURITY_CSP_MODE=strict
SECURITY_FRAME_OPTIONS=deny
SECURITY_REFERRER_POLICY=strict-origin-when-cross-origin

# Rate Limiting
RATE_LIMITER_API_REQUEST_RATE_SECONDS=1
RATE_LIMITER_API_REQUEST_BURST=100

# Logging
RUST_LOG=info
```

See [Configuration Documentation](CONFIGURATION.md) for all available options.

## Docker Deployment

### Dockerfile

Create a `Dockerfile` in the project root:

```dockerfile
# Build stage
FROM rust:1.70-slim as builder

WORKDIR /app

# Install dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Create dummy main.rs to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --bin server
RUN rm -rf src

# Copy actual source
COPY src ./src
COPY templates ./templates
COPY static ./static

# Build release binary
RUN touch src/main.rs
RUN cargo build --release --bin server

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/server /usr/local/bin/server

# Copy static assets
COPY --from=builder /app/static ./static
COPY --from=builder /app/templates ./templates

# Create uploads directory
RUN mkdir -p /app/uploads

# Expose port
EXPOSE 3000

# Run the server
CMD ["server"]
```

### Docker Compose

Create `docker-compose.yml`:

```yaml
version: '3.8'

services:
  app:
    build: .
    ports:
      - "3000:3000"
    environment:
      - DATABASE_URL=postgresql://postgres:password@db:5432/axum_kickoff
      - SESSION_KEY=${SESSION_KEY}
      - GITHUB_CLIENT_ID=${GITHUB_CLIENT_ID}
      - GITHUB_CLIENT_SECRET=${GITHUB_CLIENT_SECRET}
      - GITHUB_REDIRECT_URI=https://example.com/auth/github/callback
      - WEB_ALLOWED_ORIGINS=https://example.com
      - STORAGE_BACKEND=s3
      - STORAGE_S3_BUCKET=${S3_BUCKET}
      - STORAGE_S3_REGION=${S3_REGION}
      - STORAGE_S3_ACCESS_KEY=${S3_ACCESS_KEY}
      - STORAGE_S3_SECRET_KEY=${S3_SECRET_KEY}
      - SECURITY_HSTS_ENABLED=true
      - SECURITY_CSP_MODE=strict
      - RUST_LOG=info
    depends_on:
      - db
    restart: unless-stopped

  db:
    image: postgres:15
    environment:
      - POSTGRES_USER=postgres
      - POSTGRES_PASSWORD=password
      - POSTGRES_DB=axum_kickoff
    volumes:
      - postgres_data:/var/lib/postgresql/data
    restart: unless-stopped

volumes:
  postgres_data:
```

### Build and Run

```bash
# Build the image
docker build -t axum-kickoff .

# Run with Docker Compose
docker-compose up -d

# View logs
docker-compose logs -f app
```

## Systemd Deployment

### Build Release Binary

```bash
cargo build --release --bin server
```

The binary will be at `target/release/server`.

### Create Systemd Service

Create `/etc/systemd/system/axum-kickoff.service`:

```ini
[Unit]
Description=axum-kickoff Web Server
After=network.target postgresql.service

[Service]
Type=simple
User=axum-kickoff
Group=axum-kickoff
WorkingDirectory=/opt/axum-kickoff
Environment="RUST_LOG=info"
EnvironmentFile=/opt/axum-kickoff/.env
ExecStart=/opt/axum-kickoff/server
Restart=always
RestartSec=10

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/axum-kickoff/uploads

[Install]
WantedBy=multi-user.target
```

### Setup User and Directories

```bash
# Create user
sudo useradd -r -s /bin/false axum-kickoff

# Create directories
sudo mkdir -p /opt/axum-kickoff
sudo mkdir -p /opt/axum-kickoff/uploads
sudo mkdir -p /opt/axum-kickoff/static
sudo mkdir -p /opt/axum-kickoff/templates

# Copy files
sudo cp target/release/server /opt/axum-kickoff/
sudo cp -r static/* /opt/axum-kickoff/static/
sudo cp -r templates/* /opt/axum-kickoff/templates/

# Set permissions
sudo chown -R axum-kickoff:axum-kickoff /opt/axum-kickoff
sudo chmod 750 /opt/axum-kickoff
```

### Configure Environment

Create `/opt/axum-kickoff/.env`:

```bash
DATABASE_URL=postgresql://user:password@localhost:5432/axum_kickoff
SESSION_KEY=<your-secure-key>
# ... other environment variables
```

Set secure permissions:

```bash
sudo chmod 600 /opt/axum-kickoff/.env
```

### Start Service

```bash
# Reload systemd
sudo systemctl daemon-reload

# Enable service
sudo systemctl enable axum-kickoff

# Start service
sudo systemctl start axum-kickoff

# Check status
sudo systemctl status axum-kickoff

# View logs
sudo journalctl -u axum-kickoff -f
```

## Nginx Reverse Proxy

### Nginx Configuration

Create `/etc/nginx/sites-available/axum-kickoff`:

```nginx
upstream axum_kickoff {
    server 127.0.0.1:3000;
}

server {
    listen 80;
    server_name example.com;

    # Redirect to HTTPS
    return 301 https://$server_name$request_uri;
}

server {
    listen 443 ssl http2;
    server_name example.com;

    # SSL Configuration
    ssl_certificate /etc/letsencrypt/live/example.com/fullchain.pem;
    ssl_certificate_key /etc/letsencrypt/live/example.com/privkey.pem;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers HIGH:!aNULL:!MD5;

    # Security Headers
    add_header Strict-Transport-Security "max-age=31536000; includeSubDomains; preload" always;
    add_header X-Frame-Options "deny" always;
    add_header X-Content-Type-Options "nosniff" always;
    add_header X-XSS-Protection "1; mode=block" always;

    # Proxy Settings
    location / {
        proxy_pass http://axum_kickoff;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection 'upgrade';
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        proxy_cache_bypass $http_upgrade;
        proxy_read_timeout 86400;
    }

    # Static Files (optional)
    location /static/ {
        alias /opt/axum-kickoff/static/;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }
}
```

### Enable Site

```bash
sudo ln -s /etc/nginx/sites-available/axum-kickoff /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

### SSL with Let's Encrypt

```bash
sudo apt-get install certbot python3-certbot-nginx
sudo certbot --nginx -d example.com
```

## PostgreSQL Setup

### Install PostgreSQL

**Ubuntu/Debian:**
```bash
sudo apt-get install postgresql postgresql-contrib
```

**CentOS/RHEL:**
```bash
sudo yum install postgresql postgresql-server
sudo postgresql-setup initdb
sudo systemctl start postgresql
sudo systemctl enable postgresql
```

### Create Database and User

```bash
sudo -u postgres psql
```

```sql
CREATE DATABASE axum_kickoff;
CREATE USER axum_kickoff_user WITH ENCRYPTED PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE axum_kickoff TO axum_kickoff_user;
\q
```

### Configure Connection

Update `DATABASE_URL` in environment:

```bash
DATABASE_URL=postgresql://axum_kickoff_user:secure_password@localhost:5432/axum_kickoff
```

### Database Backups

Set up automated backups with cron:

```bash
# Backup script
cat > /opt/backups/backup-db.sh << 'EOF'
#!/bin/bash
BACKUP_DIR="/opt/backups"
DATE=$(date +%Y%m%d_%H%M%S)
pg_dump -U axum_kickoff_user axum_kickoff > $BACKUP_DIR/axum_kickoff_$DATE.sql
find $BACKUP_DIR -name "axum_kickoff_*.sql" -mtime +7 -delete
EOF

chmod +x /opt/backups/backup-db.sh

# Add to cron (daily at 2 AM)
(crontab -l 2>/dev/null; echo "0 2 * * * /opt/backups/backup-db.sh") | crontab -
```

## Cloud Platform Deployment

### AWS

#### EC2

1. Launch EC2 instance (Ubuntu 22.04)
2. Install Docker: `curl -fsSL https://get.docker.com | sh`
3. Follow Docker deployment instructions
4. Configure Security Groups (allow 80, 443)
5. Set up ELB/ALB for load balancing
6. Use RDS for PostgreSQL
7. Use S3 for storage

#### ECS

Create ECS task definition:

```json
{
  "family": "axum-kickoff",
  "containerDefinitions": [
    {
      "name": "axum-kickoff",
      "image": "your-registry/axum-kickoff:latest",
      "memory": 512,
      "cpu": 256,
      "essential": true,
      "portMappings": [
        {
          "containerPort": 3000,
          "protocol": "tcp"
        }
      ],
      "environment": [
        {
          "name": "DATABASE_URL",
          "value": "postgresql://..."
        }
      ],
      "secrets": [
        {
          "name": "SESSION_KEY",
          "valueFrom": "arn:aws:secretsmanager:..."
        }
      ]
    }
  ]
}
```

### DigitalOcean

#### App Platform

1. Connect GitHub repository
2. Configure build settings (Rust)
3. Set environment variables
4. Configure database (Managed PostgreSQL)
5. Configure storage (Spaces)
6. Deploy

#### Droplet

Follow Systemd deployment instructions on a Droplet.

### Heroku

Create `Procfile`:

```
web: server
```

Deploy:

```bash
heroku create your-app-name
heroku addons create heroku-postgresql
heroku config:set DATABASE_URL=$(heroku config:get DATABASE_URL)
heroku config:set SESSION_KEY=$(openssl rand -base64 64)
heroku config:set GITHUB_CLIENT_ID=your_client_id
heroku config:set GITHUB_CLIENT_SECRET=your_client_secret
heroku config:set GITHUB_REDIRECT_URI=https://your-app-name.herokuapp.com/auth/github/callback
heroku config:set WEB_ALLOWED_ORIGINS=https://your-app-name.herokuapp.com
git push heroku main
```

## Monitoring

### Health Checks

Add health check endpoint (if not already implemented):

```rust
pub async fn health_check() -> &'static str {
    "OK"
}
```

Configure in router:

```rust
router.route("/health", get(health_check))
```

### Logs

Use structured logging with QuickWit (see [QuickWit Integration](quickwit-integration.md)):

```bash
# Set JSON logging for production
export LOG_FORMAT=json
```

### Metrics

Enable Prometheus metrics:

```bash
cargo run --bin server --features metrics
```

Metrics available at `/metrics`.

Set up Prometheus and Grafana for visualization.

## Scaling

### Horizontal Scaling

For multiple instances:

1. Use PostgreSQL instead of SQLite
2. Use Redis for rate limiting (see [Rate Limiting Documentation](RATE_LIMITING.md))
3. Use S3 for storage
4. Set up load balancer (Nginx, HAProxy, or cloud LB)
5. Ensure session key is shared across instances

### Database Scaling

- **Read Replicas**: Configure read replicas for PostgreSQL
- **Connection Pooling**: Tune connection pool size
- **Indexing**: Add appropriate database indexes
- **Caching**: Consider Redis caching for frequently accessed data

## Security

### Firewall

Configure firewall (UFW example):

```bash
sudo ufw allow 22/tcp    # SSH
sudo ufw allow 80/tcp    # HTTP
sudo ufw allow 443/tcp   # HTTPS
sudo ufw enable
```

### Fail2Ban

Install and configure Fail2Ban:

```bash
sudo apt-get install fail2ban
sudo systemctl enable fail2ban
sudo systemctl start fail2ban
```

### Security Headers

Ensure security headers are enabled in production:

```bash
SECURITY_HSTS_ENABLED=true
SECURITY_CSP_MODE=strict
SECURITY_FRAME_OPTIONS=deny
```

## Troubleshooting

### Service Won't Start

Check logs:
```bash
sudo journalctl -u axum-kickoff -n 50
```

Common issues:
- Missing environment variables
- Database connection failed
- Port already in use
- Incorrect permissions

### Database Connection Failed

- Verify PostgreSQL is running
- Check connection string format
- Ensure database user has correct permissions
- Check firewall rules

### High Memory Usage

- Tune connection pool size
- Enable metrics to identify memory leaks
- Consider adding swap space
- Monitor with `htop` or similar

### Slow Response Times

- Check database query performance
- Enable query logging
- Add database indexes
- Consider caching with Redis
- Check network latency

## See Also

- [Configuration Documentation](CONFIGURATION.md)
- [Architecture Documentation](ARCHITECTURE.md)
- [Rate Limiting Documentation](RATE_LIMITING.md)
- [Storage Documentation](STORAGE.md)
- [QuickWit Integration](quickwit-integration.md)
