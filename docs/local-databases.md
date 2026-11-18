# Starting and Stopping Local Databases

UniDB is a database client, so local database servers (except SQLite, which reads `.db` files directly) must be running and listening on their port before clicking **Test** or **Connect**.

## Quick port check

Check which database ports are currently listening on your machine:

```bash
ss -lntup | grep -E "5432|26257|3306|6379|27017|9042|1433"
```

## Commands by engine

### PostgreSQL (`127.0.0.1:5432`)

```bash
sudo systemctl start postgresql
sudo systemctl stop postgresql
sudo systemctl status postgresql
```

### MySQL (`127.0.0.1:3306`)

```bash
sudo systemctl start mysql
sudo systemctl stop mysql
sudo systemctl status mysql
```

### MariaDB (`127.0.0.1:3306`)

```bash
sudo systemctl start mariadb
sudo systemctl stop mariadb
sudo systemctl status mariadb
```

### Redis (`127.0.0.1:6379`)

Installed via `apt` (`redis-server` systemd service):

```bash
sudo systemctl start redis-server
sudo systemctl stop redis-server
sudo systemctl status redis-server
```

Installed as a user binary (for example `~/.local/bin/redis-server`, with no systemd service):

```bash
redis-server --daemonize yes
redis-cli shutdown
```

UniDB connection settings for local Redis without auth: `Host = localhost`, `Port = 6379`, leave **Database**, **Username**, and **Password** blank (or set **Database** to `0`).

### MongoDB (`127.0.0.1:27017`)

Installed via `mongod` systemd service:

```bash
sudo systemctl start mongod
sudo systemctl stop mongod
sudo systemctl status mongod

# Optional: auto start on boot
sudo systemctl enable --now mongod
```

User-owned local instance without `sudo` (uses a directory under your home folder):

```bash
mkdir -p ~/.local/share/unidb/mongo-data
mongod --dbpath ~/.local/share/unidb/mongo-data --bind_ip 127.0.0.1 --port 27017 --fork --logpath ~/.local/share/unidb/mongo.log
mongod --dbpath ~/.local/share/unidb/mongo-data --shutdown
```

UniDB connection settings for local MongoDB without auth: `Host = localhost`, `Port = 27017`, leave **Database** blank (`admin`), and leave **Username** and **Password** blank.

### CockroachDB (`127.0.0.1:26257`)

```bash
# Single node local insecure dev server
cockroach start-single-node --insecure --listen-addr=127.0.0.1:26257 --http-addr=127.0.0.1:8080 --background
cockroach quit --insecure --host=127.0.0.1:26257
```

Or via systemd if packaged:

```bash
sudo systemctl start cockroachdb
sudo systemctl stop cockroachdb
```

### Apache Cassandra (`127.0.0.1:9042`)

```bash
sudo systemctl start cassandra
sudo systemctl stop cassandra
sudo systemctl status cassandra
```

### Microsoft SQL Server (`127.0.0.1:1433`)

```bash
sudo systemctl start mssql-server
sudo systemctl stop mssql-server
sudo systemctl status mssql-server
```

### SQLite (local `.db` file)

SQLite runs embedded inside UniDB with no background server to start or stop. Set **Engine** to `SQLite` and **Database or file** to the `.db` file path (for example `/home/user/data/app.db`).
