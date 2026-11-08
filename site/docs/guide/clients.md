# Clients

The **Clients** page shows who downloads what through the cache: the PCs downloading right now, and what each
client got over the last day, week or month, with how much of it came from the stash. lanstash counts this as it
sends the bytes, and keeps it by the minute, so it outlives a restart.

!!! note "Signed-in accounts only"
    Which PC got which game is for accounts: the Clients page asks a visitor who is not signed in to sign in.
    Elsewhere (the Overview's **Downloading now**, for example), such a visitor sees each client under a name of
    no meaning, such as `client-1` or `client-2`, without its address or name. See [Accounts](accounts.md).

## Downloading now

At the top, each client downloading at this moment gets a card per game, drawn like a launcher's download:

- the client and platform over the game's name;
- **Speed** over the last seconds;
- **This download**: what it got so far, and when it **Started**. A client that got nothing of a game for two
  minutes starts a new download of it when it asks again;
- **From cache**: how much of the last seconds came from the stash.

When nobody downloads, the panel says so.

## Over the range

Choose **Day**, **Week** or **Month** (the last 24 hours, 7 days or 30 days). Then:

- **Who got most**: a bar of each client's share of all that went to clients;
- a card per client, the one that got most first: on its left what it got, split **from the stash** and **from
  the internet**; on its right its games, largest first, each with its **cache** share.

The busiest clients are shown first; **Show all** lists the rest. **Find a client** searches by name, address,
or a game a client got.

A game's own page has the same view for that game: **Who got it**, over the last 30 days.

## Naming a client

A client is called by the name you gave it, else by what your network's DNS calls its address, else by its
address (for example `192.0.2.20`).

To name one, choose **Rename** on its card, type a name such as `PC-1` or `Office`, and **Save**. An empty name
takes the given name away, and the client goes back to the network's name. Naming clients needs an account that
may name them (an admin).

!!! tip
    lanstash asks your network's DNS servers for the names of the addresses it sees (reverse DNS), so PCs your
    router already names show by that name without any typing. `client_names` in the configuration says which
    servers to ask, or `off`.
