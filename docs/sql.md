# SQL & PostgreSQL Learning Plan

A structured plan from SQL fundamentals through PostgreSQL DBA-level skills. Tailored for a software engineer with cloud-native and DevOps experience.

---

## Phase 1: SQL Foundations (1–2 weeks)

**Goal:** Solidify core SQL syntax, querying patterns, and relational thinking.

Since you already code daily, this phase is about getting fluent in SQL's mental model — thinking in sets, not loops.

### Interactive Practice (pick one primary)

| Resource | Format | Link |
|----------|--------|------|
| **SQLBolt** | Browser exercises, no setup | [sqlbolt.com](https://sqlbolt.com) |
| **SQLZoo** | Progressive challenges with real datasets | [sqlzoo.net](https://sqlzoo.net) |
| **W3Schools SQL** | Reference + try-it-yourself editor | [w3schools.com/sql](https://www.w3schools.com/sql/) |
| **freeCodeCamp Relational DB Cert** | CLI-based, uses PostgreSQL + Git in VS Code | [freecodecamp.org/learn/relational-database](https://www.freecodecamp.org/learn/relational-database/) |

### Video Courses

| Resource | Duration | Link |
|----------|----------|------|
| **Harvard CS50 SQL** (freeCodeCamp) | ~11 hours | [YouTube](https://www.youtube.com/watch?v=wHGpJjM3Rl0) |
| **SQL Tutorial for Beginners** (Mosh) | ~3 hours | [YouTube](https://www.youtube.com/watch?v=7S_tz1z_5bA) |
| **Khan Academy — Intro to SQL** | Self-paced | [khanacademy.org](https://www.khanacademy.org/computing/computer-programming/sql) |

### Topics to Cover

- SELECT, WHERE, ORDER BY, GROUP BY, HAVING
- JOINs (INNER, LEFT, RIGHT, FULL, CROSS, SELF)
- Subqueries and CTEs (Common Table Expressions)
- Aggregate functions (COUNT, SUM, AVG, MIN, MAX)
- UNION, INTERSECT, EXCEPT
- INSERT, UPDATE, DELETE
- CREATE TABLE, ALTER TABLE, constraints (PK, FK, UNIQUE, CHECK, NOT NULL)
- CASE expressions
- NULL handling (COALESCE, NULLIF, IS NOT DISTINCT FROM)

---

## Phase 2: PostgreSQL Specifics (2–3 weeks)

**Goal:** Learn what makes Postgres different — its type system, extensions, and developer ergonomics.

### Core Courses

| Resource | Format | Link |
|----------|--------|------|
| **Mastering Postgres** — Aaron Francis | Video course (much of it free, sponsored by Supabase) | [masteringpostgres.com](https://masteringpostgres.com) |
| **PostgreSQL for Everybody** (Coursera / UMich) | 4-course specialization by Charles Severance | [coursera.org](https://www.coursera.org/specializations/postgresql-for-everybody) |
| **PostgreSQL Tutorial** (freeCodeCamp, Amigoscode) | ~4 hours, beginner-friendly | [YouTube](https://www.youtube.com/watch?v=qw--VYLpxG4) |
| **PostgreSQL Beginner Course** (freeCodeCamp) | ~3 hours | [YouTube](https://www.youtube.com/watch?v=SpfIwlAYaKk) |

### Official Docs (bookmark these)

| Resource | Link |
|----------|------|
| **PostgreSQL Official Tutorial** | [postgresql.org/docs/current/tutorial.html](https://www.postgresql.org/docs/current/tutorial.html) |
| **PostgreSQL Official Docs** | [postgresql.org/docs/current](https://www.postgresql.org/docs/current/) |
| **PostgreSQL Wiki** | [wiki.postgresql.org](https://wiki.postgresql.org) |

### Topics to Cover

- Postgres-specific data types: JSONB, arrays, hstore, INET, UUID, ENUM, ranges, tsvector
- Sequences and identity columns
- `EXPLAIN` / `EXPLAIN ANALYZE` — reading query plans
- Window functions (ROW_NUMBER, RANK, LAG, LEAD, OVER, PARTITION BY)
- Full-text search
- Generated columns, exclusion constraints
- Views and materialized views
- Stored functions (PL/pgSQL basics)
- Triggers
- psql CLI mastery (`.dt`, `.di`, `.d+`, `\timing`, `\x`, `\copy`)

---

## Phase 3: Database Design & Modeling (1–2 weeks)

**Goal:** Design schemas that perform well and evolve cleanly.

### Resources

| Resource | Format | Link |
|----------|--------|------|
| **Database Design Course** (freeCodeCamp) | ~8 hours | [YouTube](https://www.youtube.com/watch?v=ztHopE5Wnpc) |
| **PostgreSQL Tutorial** (postgresqltutorial.com) | Text + examples | [postgresqltutorial.com](https://www.postgresqltutorial.com/) |

### Topics to Cover

- Normalization (1NF through 3NF, BCNF) and when to denormalize
- Entity-Relationship modeling
- Choosing primary keys (serial vs bigserial vs UUID vs ULID)
- Foreign key design and cascading behavior
- Indexing strategy at design time
- Partitioning (range, list, hash) — when and how
- Schema migration workflows (you already know Atlas — apply it here)
- Multi-tenancy patterns (schema-per-tenant vs shared tables with RLS)

---

## Phase 4: Indexing & Query Performance (2–3 weeks)

**Goal:** This is where the real leverage is. Learn to make Postgres fast.

### Resources

| Resource | Format | Link |
|----------|--------|------|
| **Mastering Postgres** — Indexing chapters | Video | [masteringpostgres.com](https://masteringpostgres.com) |
| **Use The Index, Luke** | Free online book | [use-the-index-luke.com](https://use-the-index-luke.com) |
| **EDB Performance Tuning Guide** | Comprehensive article | [enterprisedb.com](https://www.enterprisedb.com/postgres-tutorials/introduction-postgresql-performance-tuning-and-optimization) |
| **Percona PG Tuning Guide** | Blog (2025 updated) | [percona.com](https://www.percona.com/blog/tuning-postgresql-database-parameters-to-optimize-performance/) |
| **pganalyze YouTube** | Short focused videos | [youtube.com/@pganalyze](https://www.youtube.com/@pganalyze) |

### Topics to Cover

- Index types: B-tree, Hash, GIN, GiST, BRIN, SP-GiST
- Composite indexes and column ordering
- Partial indexes and expression indexes
- Covering indexes (INCLUDE)
- Index-only scans
- HOT updates and how indexes affect write performance
- `EXPLAIN (ANALYZE, BUFFERS, FORMAT YAML)` deep dive
- `pg_stat_statements` for identifying slow queries
- `pg_stat_user_indexes` for identifying unused indexes
- Query planner internals (cost estimation, selectivity)
- Connection pooling with PgBouncer

---

## Phase 5: PostgreSQL Administration (2–4 weeks)

**Goal:** Operate Postgres in production — backups, replication, monitoring, security.

### Resources

| Resource | Format | Link |
|----------|--------|------|
| **PostgreSQL Administration** (Udemy — various) | Paid courses | [udemy.com/topic/postgresql](https://www.udemy.com/topic/postgresql/) |
| **PostgreSQL Quickies** (SQLpassion) | Short YouTube episodes | [sqlpassion.at](https://www.sqlpassion.at/archive/2025/09/15/introducing-my-new-postgresql-quickies-series-on-youtube/) |
| **Crunchy Data YouTube** | Production-focused talks | [youtube.com/@CrunchyDataPostgres](https://www.youtube.com/@CrunchyDataPostgres) |
| **CYBERTEC PostgreSQL YouTube** | Deep technical content | [youtube.com/@cybaboread](https://www.youtube.com/@cybaboread) |
| **pganalyze Blog** | Performance & admin articles | [pganalyze.com/blog](https://pganalyze.com/blog) |

### Topics to Cover

**Configuration & Tuning:**
- `postgresql.conf` key parameters: `shared_buffers`, `work_mem`, `maintenance_work_mem`, `effective_cache_size`, `max_connections`, `wal_level`
- WAL configuration: `checkpoint_timeout`, `max_wal_size`, `wal_buffers`
- Autovacuum tuning: thresholds, scale factors, cost limits
- PGTune tool for generating configs: [pgtune.leopard.in.ua](https://pgtune.leopard.in.ua/)

**Backup & Recovery:**
- `pg_dump` / `pg_dumpall` (logical backups)
- `pg_basebackup` (physical backups)
- Point-in-Time Recovery (PITR) with WAL archiving
- pgBackRest for production backups
- Testing restore procedures

**Replication & HA:**
- Streaming replication (sync and async)
- Logical replication (selective table/schema replication)
- Patroni for automatic failover + etcd/Consul
- pgpool-II / HAProxy for connection routing
- Read replicas for scaling reads

**Monitoring:**
- `pg_stat_activity`, `pg_stat_database`, `pg_stat_user_tables`
- Prometheus + Grafana with `postgres_exporter`
- pgBadger for log analysis
- Deadlock detection and lock monitoring

**Security:**
- `pg_hba.conf` — authentication rules
- Role-based access control (GRANT/REVOKE)
- Row-Level Security (RLS)
- SSL/TLS connections
- Encryption at rest considerations

---

## Phase 6: Advanced & Production Patterns (ongoing)

**Goal:** Patterns you'll reach for in real production systems.

### Resources

| Resource | Format | Link |
|----------|--------|------|
| **Scaling Postgres** (podcast/YouTube) | Weekly episodes | [scalingpostgres.com](https://www.scalingpostgres.com/) |
| **PostgreSQL Wiki: Don't Do This** | Anti-patterns list | [wiki.postgresql.org/wiki/Don't_Do_This](https://wiki.postgresql.org/wiki/Don%27t_Do_This) |
| **Postgres FM** (podcast) | Weekly conversations | [postgres.fm](https://postgres.fm/) |

### Topics to Cover

- MVCC internals (how Postgres handles concurrency)
- Transaction isolation levels and their tradeoffs
- Advisory locks
- Table partitioning strategies at scale
- Bulk loading (COPY vs INSERT, `pg_bulkload`)
- Foreign Data Wrappers (FDW) for querying external data sources
- Logical decoding and Change Data Capture (CDC)
- Extensions ecosystem: `pg_cron`, `pgvector`, `PostGIS`, `pg_partman`, `timescaledb`
- Kubernetes operators for Postgres: CloudNativePG, Crunchy PGO, Zalando Postgres Operator

---

## Practice Platforms

Keep your SQL sharp with regular drills:

| Platform | Link |
|----------|------|
| **HackerRank SQL** | [hackerrank.com/domains/sql](https://www.hackerrank.com/domains/sql) |
| **LeetCode Database** | [leetcode.com/problemset/database](https://leetcode.com/problemset/database/) |
| **DataLemur** | [datalemur.com](https://datalemur.com) |
| **SQLZoo** | [sqlzoo.net](https://sqlzoo.net) |
| **pgexercises** | [pgexercises.com](https://pgexercises.com/) |

---

## Books (Optional Deep Dives)

| Book | Why |
|------|-----|
| **The Art of PostgreSQL** — Dimitri Fontaine | Teaches you to push logic into Postgres instead of app code |
| **PostgreSQL 14 Internals** — Egor Rogov (free PDF) | Deep understanding of MVCC, WAL, buffer cache, query planning |
| **SQL Antipatterns** — Bill Karwin | Learn what NOT to do |

---

## Suggested Weekly Schedule

| Week | Focus | Time |
|------|-------|------|
| 1 | SQL foundations (SQLBolt + CS50 SQL videos) | ~6h |
| 2 | JOINs, subqueries, CTEs, window functions | ~6h |
| 3 | Mastering Postgres course (data types, basics) | ~5h |
| 4 | Mastering Postgres (indexes, EXPLAIN) + practice | ~5h |
| 5 | Database design + schema migrations | ~5h |
| 6 | Performance tuning (Use The Index Luke + EDB guide) | ~5h |
| 7 | Admin: config tuning, backup, recovery | ~5h |
| 8 | Admin: replication, monitoring, security | ~5h |
| 9+ | Advanced patterns, practice problems, real projects | ongoing |

---

## Quick-Start Command

Spin up Postgres locally in 10 seconds:

```bash
docker run --name pg-learn -e POSTGRES_PASSWORD=learn -p 5432:5432 -d postgres:17
docker exec -it pg-learn psql -U postgres
```

Then start with SQLBolt or jump straight into Mastering Postgres. You've got the engineering intuition — this plan is about filling in the SQL-specific gaps and going deep on Postgres internals.