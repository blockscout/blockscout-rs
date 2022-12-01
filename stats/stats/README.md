# Stats service


## Development

> all commands should be executed in `stats/stats` dir

### Postgres

```bash
docker run -p 5432:5432 --name stats-postgres -e POSTGRES_PASSWORD=admin -d postgres
docker exec -it stats-postgres psql -U postgres -c 'create database stats;'
export DATABASE_URL=postgres://postgres:admin@localhost:5432/stats
```

### Migrations

1. Install `sea-orm-cli`:

```bash
cargo install sea-orm-cli
```

2. Get current status:

```bash
sea-orm-cli migrate status
```

3. Apply migrations:

```bash
sea-orm-cli migrate up
```

4. Downgrade by 1 migration:

```bash
sea-orm-cli migrate down
```

5. Generate new migration:

```bash
sea-orm-cli migrate generate <migration name>
```

### Code gen

1. Generate sea-orm database entities:

```bash
sea-orm-cli generate entity --lib -o entity/src
```
