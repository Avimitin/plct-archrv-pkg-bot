-- Add migration script here
CREATE TABLE IF NOT EXISTS packager(
  tg_uid INT NOT NULL UNIQUE PRIMARY KEY,
  alias  TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pkg (
  id               INTEGER PRIMARY KEY,
  name             TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS assignment (
  id          INTEGER PRIMARY KEY,
  pkg         INTEGER NOT NULL,
  assignee    INT NOT NULL,
  assigned_at INT NOT NULL,
  FOREIGN KEY(pkg) REFERENCES pkg(id),
  FOREIGN KEY(assignee) REFERENCES packager(tg_uid)
);

CREATE TABLE IF NOT EXISTS mark(
  name      TEXT NOT NULL,
  marked_by INT,
  marked_at INT NOT NULL,
  msg_id    INT NOT NULL,
  comment   TEXT,
  for_pkg   INTEGER,

  FOREIGN KEY(marked_by) REFERENCES packager(tg_uid),
  FOREIGN KEY(for_pkg) REFERENCES pkg(id)
);

CREATE TABLE IF NOT EXISTS pkg_relation(
  -- outdated_dep, missing_dep...etc
  status       TEXT NOT NULL,
  -- if missing_dep, the dep pkg
  required     INT NOT NULL,
  -- which package require the `required` to be ready
  request      INT NOT NULL,

  FOREIGN KEY(required) REFERENCES pkg(id),
  FOREIGN KEY(request) REFERENCES pkg(id)
);
