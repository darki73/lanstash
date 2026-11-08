# Accounts

Anyone on your network may open the web UI and look. Changing things (prefilling, deleting games, changing
settings, naming clients) needs a lanstash account with a role that allows it. This page explains the roles,
how to make the first admin and invite others, how a player gets its own games, and how to serve the web UI over
HTTPS.

## Roles and permissions

Each account has one role:

| Role | In short |
|---|---|
| **Admin** | May do everything, and sees hidden games. |
| **Player** | Prefills the games its platform accounts own. |
| **Viewer** | Reads everything, with hidden games masked. |

What each permission allows, and who holds it:

| Permission | Admin | Player | Viewer |
|---|:-:|:-:|:-:|
| Prefill any game, stop a prefill and change the queue | yes | | |
| Prefill the games your linked platform accounts own | yes | yes | |
| Change the settings and restart lanstash | yes | | |
| Set up the DNS server and apply its rewrites | yes | | |
| Delete and prune games in the stash | yes | | |
| Hide games, and see the hidden ones | yes | | |
| Name the clients | yes | | |
| Sign lanstash in to platforms (Steam, Epic) | yes | | |
| Manage lanstash's accounts and invites | yes | | |

Reading is open to everyone, with two exceptions: the Clients page needs a signed-in account (any role), and the
DNS page needs the DNS permission. The web UI shows each button only to an account that may use it; a player
gets no Stop button, for example.

!!! warning "This machine may do everything"
    A request from lanstash's own machine holds every permission, so commands such as `lanstash prefill`,
    `lanstash stop` and `lanstash doctor` need no sign-in. For the same reason, do not put a reverse proxy in
    front of the admin port on the same machine: everyone it relays would be an admin. Run a proxy elsewhere.

## Signing in

**Sign in** sits at the foot of the sidebar. Sign in with your name and password; **Your account** (at the same
place once signed in) shows your role, what you may do, your linked platform accounts, and lets you change your
password. A sign-in lasts 30 days from its last use. Changing your password signs out every page signed in as
you.

After a wrong password, the next try from the same address has to wait a moment, longer with each wrong try (up
to 30 seconds).

## The first admin

Accounts are made on lanstash's own machine first. Make the first admin there:

```sh
sudo lanstash user add admin --admin
```

lanstash asks for the password twice (10 to 128 characters). A name is 1 to 32 of `a-z`, `0-9`, dot, dash and
underscore. More commands:

```sh
sudo lanstash user list                  # every account and its role
sudo lanstash user passwd admin          # set a new password
sudo lanstash user role player.one admin # make an account an admin (or a viewer)
sudo lanstash user remove player.one     # remove an account
```

`user add` without `--admin` makes a viewer. The last admin can be neither removed nor made a viewer. Passwords
are kept only as hashes.

## Adding accounts and invites

Once signed in as an admin, open **Accounts** in the sidebar. It lists every account with its role, which you can
switch in place, and a player's platform accounts. **Add an account** offers two ways:

- **They set it (invite link)**: choose a **Role** and, if you like, a **Name** (leave it empty and they choose
  one). **Make an invite link** gives a link to send them. It works once, for a week, and is shown only once.
  Opening it shows **Join lanstash**, where they choose a password (and a name, if you left it open); **Make the
  account** creates the account and signs them in.
- **Set it now**: type a name and a password, and **Add the account**. The account may sign in at once; tell them
  the password.

**Invites waiting** lists the invites not used yet; **Take back** cancels one.

## Linking a platform account to a player

A player prefills only its own games: those owned by the platform accounts linked to it. To link one:

1. Sign lanstash in to the Steam account on the Steam page's **Account** part (see
   [Platforms](platforms.md#what-each-platform-needs)).
2. On **Accounts**, add that Steam account to the player's **Platform accounts**, or choose it under **Platform
   accounts** when you add the player or make its invite.

The player's Library then lists only the games those accounts own, and it may prefill those. **Your account**
lists the linked platform accounts, under **Linked platform accounts**. A player may also keep its own games to
itself: see [Hidden and private games](privacy.md#private-games).

## What a signed-out visitor sees

A visitor who is not signed in reads like a viewer: the Overview, platforms, games, History and Health, with
hidden games masked. They do not see:

- the Clients page, nor which PC got which game: elsewhere, clients show as `client-1`, `client-2` and so on;
- the DNS page;
- any button that changes something.

## HTTPS for the web UI

By default the web UI is plain HTTP, so the sign-in and its session cookie cross your LAN unencrypted. To serve
it over HTTPS, set `admin_listen_tls` in the configuration, for example:

```ini
admin_listen_tls = 0.0.0.0:8443
```

lanstash makes a self-signed certificate once and keeps it with its secrets (it makes a new one when its names
change). The plain `admin_listen` port then still answers lanstash's own machine, and sends every other request
to the same page over HTTPS. Because the certificate is self-signed, browsers warn about it the first time; accept
it to go on. See the [configuration reference](../reference/configuration.md).
