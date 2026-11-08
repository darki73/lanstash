# Map: DNS server

How lanstash sets up and checks the network's DNS server (AdGuard Home, Pi-hole), which sends the game CDN names to
the cache. Code references are `path:line`; line numbers may have moved, so search for the named item. Claims are
marked **[verified]** (read in the code or covered by a test) or **[hypothesis]**. The use case is
`crates/app/src/dns` (`DnsLink`), the ports `crates/ports/src/dns.rs`, the adapters `crates/dns/adguard` and
`crates/dns/pihole`, the kept setup `crates/server/src/dns.rs`; wired by `dns_link`
`crates/server/src/run/serve.rs:273`.

## Ports

`crates/ports/src/dns.rs`. A rewrite's pattern is written as in the cache-domains lists: `name` is that one name,
`*.name` its subdomains, not the name itself; each adapter turns it into its own rules.

| Item | What |
|---|---|
| `DnsKind` `:15` | a kind of server: `id` (`adguard`), `name`, usual `port` and `https_port`, `user` (signs in with a user name, else a password alone) |
| `DnsConfig` `:29` | `kind`, `host`, `port`, `https`, `pin` (SHA-256 of the one trusted certificate), `unchecked` (no certificate check), `user`, `answer` (lanstash's address, none: inferred), `via` (a name the game names point at, which points at lanstash) |
| `DnsSetup` `:49` | the kept setup: `kinds`, `config`, `password`, `save`, `forget`, `connect` (a `DnsServer` for a config, none for an unknown kind) |
| `Rewrite` `:77` | `pattern`, `answer` (an address or a name) |
| `Lookup` `:86` | a query log entry: `name`, `client`, `answers`, `rewritten`, `at_ms` |
| `Answered` `:101` | how the server answers a name now: `to` (address or name; none: upstream or blocked), `rule` (as written; none for a rewrite) |
| `DnsServer` `:109` | `kind`, `rewrites` (all, not only lanstash's), `add`, `remove`, `lookups(limit)`, `check(name)` (default `None`: cannot say, `:141`) |

## Wanted names

`patterns` `crates/server/src/dns.rs:26`: each cache-domains list's lines as written (`name`, `*.name`; comments
after `#` and blanks left out, lower-cased), then for each built-in host both `host` and `*.host`; each once, in
that order **[verified: `the_patterns_are_the_lists_as_written_and_the_built_in_hosts_with_their_subdomains`,
`crates/server/tests/dns.rs`]**. The built-in hosts are Epic's content hosts, only when `epicgames` is among the
services; Steam's are not added, as a Steam client that finds `lancache.steamcontent.com` pointing at a cache sends
all its downloads there (`dns_link` `crates/server/src/run/serve.rs:273`) **[verified in code; the Steam client
behaviour: hypothesis, as the comment states it]**. `DnsLink::new` `crates/app/src/dns/mod.rs:172` drops
duplicates (sorted set).

## The plan

`DnsLink::plan_for` `crates/app/src/dns/mod.rs:199` against the server's current rewrites gives a `Plan` `:46`
**[verified: `crates/app/tests/dns/main.rs`]**.

- What the rewrites answer (`answer`, `answer_from` `AnswerFrom` `:37`): the configured `answer`
  (`Config`), else the address the server's rewrites of wanted patterns already answer most (`Rewrites`, a vote);
  none of either is an error ("set lanstash's address"). A name answer is followed to an address through the
  rewrites (`resolve` `:147`: exact before wildcard, longest wildcard first, at most 8 steps); a configured name
  that leads to no address is an error.
- `via`: when set and its rewrite leads to an address, that address is lanstash's (unless `answer` is set). A
  `via` with no rewrite yet is `via_missing` (Apply adds it first); one that leads elsewhere is an error.
- `write_as` (what new rewrites answer): `via`, else a configured name, else the configured address, else the
  answer most of lanstash's existing wanted rewrites use (a host name when they use one).
- Lanstash's rewrites: those whose answer resolves to lanstash's address, through names too.
- `missing`: wanted patterns with none of lanstash's rewrites. `duplicates`: wanted patterns with more than one.
  `extra`: patterns answered with lanstash's address that are not wanted and are not a name the answers go through
  (reported only). `services`: each wanted pattern with the service whose host rules take it (`covered::sample`),
  else `other`.
- `covered` `crates/app/src/dns/covered.rs:21` (`split`): each missing pattern is asked about
  (`DnsServer::check` of `sample` `:12`: the name itself, or `lanstash.<base>` for `*.<base>`); when the server
  answers it with lanstash's address anyway (a rule of its own, a filter list, a wider rewrite) it moves to
  `covered` with the rule that does, or `another rewrite` (`BY_REWRITE` `:17`). A server that cannot say, or fails,
  leaves it missing **[verified: `crates/app/tests/dns/covered.rs`]**.

## Report

`DnsLink::report` `crates/app/src/dns/mod.rs:307` (`Report` `:83`): the server's kind, the plan (or why not), and
from the latest `LOOKUPS` (`:33`, 5,000) lookups the names a cached service owns that no rewrite answered (they
went to the internet), by name, most first, with at most 5 clients each (`Missed` `:72`); the oldest lookup's time;
and the hosts lanstash itself refused since its start (`Cache::refused`, `crates/app/src/serve/refused.rs`: names
the server sends to the cache that no host list names) **[verified:
`says_which_cached_names_went_to_the_internet_and_which_hosts_lanstash_refused`]**.

## Apply

`DnsLink::apply` `crates/app/src/dns/mod.rs:351` (`Applied` `:101`: `added`, `deduplicated`): `via` first when
missing (so it leads to lanstash before the names do); for each duplicate every copy that leads to lanstash is
removed, then one is added again (a server removes every copy at once); then each missing pattern is added,
answering `write_as`. It never removes a rewrite lanstash does not want: one for another address, or one for
lanstash's host meant for something else, stays. What was done before a failure stays done **[verified in code and
`plans_from_the_address_the_rewrites_already_answer_and_leaves_others_alone`]**.

## Setup check

`check` `crates/app/src/dns/setup.rs:39` tidies a `DnsConfig` or says what is wrong
**[verified: `crates/app/tests/dns/setup.rs`]**:

- Kind: one of `DnsSetup::kinds` (else the known ones are listed).
- Host: trimmed, lower case, 1 to 253 bytes of letters, digits and `-.:[]` (`host_ok` `:23`), so no scheme and no
  path. Never this machine's loopback: `localhost`, `*.localhost`, a loopback or unspecified address (IPv4-mapped
  too, `to_canonical`), brackets stripped first (`:50`) **[verified: `the_server_is_never_this_machines_loopback`]**.
- Port: not 0.
- User: only for a kind with `user` (else dropped), then needed; no control characters and no `:` (`:66`: it is
  kept one setting per line and sent before a `:` in basic authentication) **[verified:
  `a_user_name_holds_no_line_break_or_colon`]**.
- `answer`: an IPv4 address (four numbers up to 255) or IPv6 (`address_ok` `:28`), or a host name with a dot and a
  letter. `via`: a host name (trailing dot dropped, lower case).
- TLS: `pin` and `unchecked` count only with `https`. A pin is normalised by `pin` `:107` (hex kept, upper case,
  pairs joined by `:`) and must be 32 pairs (95 characters).

## Configure, test, forget

`DnsLink::configure` `crates/app/src/dns/setup.rs:120`: checks, then tries the setup (lists the server's rewrites,
`Tried` `:16`: kind and count) and keeps it only if that works and it is not a test **[verified:
`only_a_setup_that_works_is_kept_and_a_test_keeps_nothing`]**. A password left out uses the kept one only for the
same server: same kind, host, port, `https`, pin, `unchecked` and user (`same_server` `:123`); for any other it is
asked for again ("a password is needed for another server"), so a kept password is never sent to a host it was not
kept for **[verified: `the_kept_password_goes_only_to_the_server_it_was_kept_for`]**. `forget` `:145` removes the
setup and its password. `DnsLink::server` `crates/app/src/dns/mod.rs:187` connects per request with the setup as it
is then, so a change applies at once; none set up is `NOT_SET_UP` `:122`.

## Where the setup is kept

`LiveDns` `crates/server/src/dns.rs:39` implements `DnsSetup`:

- The setup: `key = value` lines (`write` `:92`, `read` `:60`; none without a kind and a host) under `dns`
  (`KEPT` `:18`) through the `Keep` port **[verified: `a_setup_is_kept_in_lines_and_the_password_apart`,
  `crates/server/tests/dns.rs`]**.
- The password: apart, under `dns-password` (`PASSWORD` `:20`) through the `Secrets` port; `save` `:124` writes
  the password first, then the setup; `forget` `:129` removes the password and empties the setup.
- The setup is a `PrivateDir` on the `state` directory, the password one on the `secrets` directory (`dns_link`
  `crates/server/src/run/serve.rs`); a password found in `state` is moved to `secrets` when first read
  (`LiveDns::password`) **[verified: `a_password_found_beside_the_setup_moves_to_the_secrets`]**. `PrivateDir` makes
  the directory 0700 and each file 0600 on Unix, written to a temporary name and renamed
  (`crates/adapters/fs/src/private_dir.rs`) **[verified in code]**.
- Kinds `:108`: `adguard` (AdGuard Home, ports 3000 and 443, user and password) and `pihole` (Pi-hole, ports 80
  and 443, password alone). `connect` `:134` builds an `Endpoint` (host, port, https, pin, unchecked) and the
  adapter; both speak only through the `Web` port (`HttpWeb`).

## AdGuard Home adapter

`AdGuard` `crates/dns/adguard/src/server.rs:33` (`no_std`, no I/O of its own), written against AdGuard Home
v0.107.62 **[verified: `crates/dns/adguard/tests/server.rs`, a fake `Web`]**:

- HTTP basic authentication (`new` `:48`); the `Debug` form never shows the credentials. 401 or 403: refused; a
  400 saying HTTPS, or a redirect to `https://`, over plain HTTP: "HTTPS-only" (`call` `:54`).
- `rewrites` `:118`: `GET /control/rewrite/list`, each `{"domain","answer"}` is a pattern as it is.
- `add` / `remove` `:128`, `:132`: `POST /control/rewrite/add` / `/control/rewrite/delete`. Adding one that is
  there makes a second copy; deleting removes every copy (hence Apply's deduplication).
- `lookups` `:136`: `GET /control/querylog?limit=N`; `rewritten` when `reason` starts with `Rewrite`, or is
  `FilteredBlackList` with a real (not `0.0.0.0` / `::`) address, a hosts-file rule (`lookup` `:91`). Times are
  RFC 3339 read by `time::ms` `crates/dns/adguard/src/time.rs:21` (`Z` or an offset).
- `check` `:141`: `GET /control/filtering/check_host?name=`, only for host names (letters, digits, `.`, `-`);
  read by `answered` `crates/dns/adguard/src/check.rs:43`: `Rewrite*` answers its address or CNAME and no rule;
  `RewriteRule` its address, CNAME, or the value of `$dnsrewrite=` (short or `NOERROR;A;` form); `FilteredBlackList`
  the address of a hosts-file rule (blocked: none); anything else none **[verified:
  `crates/dns/adguard/tests/check.rs`]**.
- An untrusted HTTPS certificate's SHA-256 stays in the error text (`untrusted_in`
  `crates/ports/src/web.rs:78`) **[verified:
  `speaks_https_trusting_a_pin_or_nothing_and_names_an_untrusted_certificate`]**.

## Pi-hole adapter

`PiHole` `crates/dns/pihole/src/server.rs:34`, Pi-hole's v6 API, written against Pi-hole 2026.10.0
**[verified: `crates/dns/pihole/tests/server.rs`, `rules.rs`]**:

- Sessions: `POST /api/auth` with the password, the `sid` sent as `X-FTL-SID`; each operation signs in and out
  again (`session` `:112`; Pi-hole keeps few sessions) **[verified:
  `rewrites_to_an_address_are_reply_rules_and_every_session_ends`]**.
- Reply rules (`crates/dns/pihole/src/rules.rs`): a rewrite to an address is a regex deny rule with a reply,
  `^name$;reply=ADDRESS` or `[.]name$;reply=ADDRESS` for `*.name`, dots as `[.]` (`regex` `:43`), posted with the
  comment `lanstash` (`COMMENT` `:48`, `rule` `:153`). It answers A with the address and AAAA with `::`, so IPv6
  clients cannot go around the cache; lanstash writes no local DNS record for an address (that would let AAAA go
  to the internet). `from_regex` `:59` also reads rules a person wrote with `\.`.
- Local records and CNAMEs: `rewrites` `:192` also reads `dns.hosts` lines (`ADDRESS name ...`, `from_host` `:80`)
  and `dns.cnameRecords` (`name,target`, `from_cname` `:88`). A rewrite to a name (`add` `:201`) takes the address
  that name has in Pi-hole (its local record, else a reply rule); a `*.name` gets a reply rule to that address
  (Pi-hole has no wildcard CNAMEs); a whole name gets a CNAME, and the target, which Pi-hole follows only when it has
  a local record and no deny rule, gets a record and loses lanstash's reply rule. A reply rule of the user's own for
  the target stays, and then the name answers the address itself **[verified:
  `names_pointed_at_a_name_are_cnames_to_a_local_record_and_wildcards_take_its_address`,
  `a_name_pointed_at_a_name_the_user_has_a_rule_for_answers_its_address_itself`]**.
- Only lanstash's own rules are removed (`remove` `:248`): a reply rule goes only when it is in effect for that
  rewrite and carries lanstash's comment or is written exactly as lanstash writes it (`Regex::ours_for` `:70`); one
  with options of its own is the user's **[verified: `a_rule_of_the_users_own_is_left_alone`]**.
- A hosts line that names several names: the line without the name is written first, then the old line is
  deleted, so a failure loses none of the other names (`without` `:180`, `remove` `:254`) **[verified:
  `a_name_is_taken_out_of_a_local_record_that_names_several`]**.
- `lookups` `:268`: `GET /api/queries?length=N`; `rewritten` for `REGEX` with the id of a replying rule, or
  `CACHE` without `upstream` for a name with a local record or CNAME; no answered addresses.
- No `check`: Pi-hole uses the default (`None`), so nothing is ever `covered` with it **[verified in code]**.
- Paths are percent-encoded (`encode` `rules.rs:96`). `misc.dnsmasq_lines` cannot be set through the API, so
  wildcards are reply rules.

## Admin API

`crates/adapters/admin-api/src/api/dns.rs`; routes in `crates/adapters/admin-api/src/route.rs` (`Route::Dns`,
`DnsApply`, `DnsSetup`, `DnsForget`, `:46` to `:52`; paths `:104`, `:111`, `:112`; `DELETE` to `DnsForget`
`:140`). Without the DNS link wired every path answers 404.

| Path | What |
|---|---|
| `GET /api/v1/dns` | `{"setup","report","report_error","done"}` (`page` `:135`). `setup`: `{"kinds":[{"id","name","port","https_port","user"}],"config":{"kind","host","port","https","pin","unchecked","user","answer","via"}\|null,"password_set"}`, never the password (`render_setup` `:41`). `report`: `{"server","plan","plan_error","missed":[{"name","service","count","clients"}],"lookups","oldest_ms","lookups_error","refused":[{"host","count"}],"refused_others"}`; `plan`: `{"answer","write_as","via","via_missing","services":{pattern:service},"answer_from":"config"\|"rewrites","wanted","missing","covered":{pattern:rule},"duplicates","extra"}` (`render` `:83`) |
| `PUT /api/v1/dns/setup?kind=&host=&port=&user=[&answer=][&via=][&https=1[&pin=\|&unchecked=1]][&test=1]` | checks and tries, keeps unless `test=1`; 200 with the page and `done: {"tested","kept","server","rewrites"}`; 400 with why; 400 `{"error","kind":"untrusted","certificate"}` when HTTPS did not trust the certificate (`dns_setup` `:170`) |
| `DELETE /api/v1/dns/setup` | forgets the setup and its password; the page |
| `POST /api/v1/dns/apply` | applies; the page with `done: {"added","deduplicated"}`; 502 when the server could not be asked (`dns` `:155`) |

The password comes only in the `X-Lanstash-Password` request header (`PASSWORD_HEADER` `:29`, named in lower case as the server gives header names),
never in the path or query; none means the kept one. The header is read only for the setup routes
(`crates/adapters/admin-api/src/api/mod.rs:371`) **[verified:
`the_dns_server_is_set_up_tested_kept_and_forgotten`, `crates/adapters/admin-api/tests/api/dns.rs`]**.
