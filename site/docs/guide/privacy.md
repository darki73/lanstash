# Hidden and private games

Some games are nobody else's business. lanstash has two ways to keep them out of sight: an admin can **hide** a
game from everyone, and a player can keep **its own** games **private**. Either way the game stays cached, is kept
up to date and can be prefilled; only its name, art, page and builds are masked.

## Hidden games

An admin hides a game from its page: the **Privacy** panel's **Hide game** button. From then on everyone who may
not hide games (players, viewers and visitors who are not signed in) sees it as **"A hidden game"**:

- without its name, art or page;
- without its builds and build numbers (a build number would tell which game it is);
- in History, the palette, the prefill queue and every list, the same way.

Its size still counts in the totals. Admins see hidden games by name, marked **hidden**.

To show a game again, use **Show to everyone** on its page, or in the **Hidden games** list on the **Settings**
page, which lists every hidden game.

## Private games

A player can keep to itself that it has a game. On the page of one of its own games (owned by a Steam account
linked to it, see [Accounts](accounts.md#linking-a-platform-account-to-a-player)), the **Keep it to myself**
panel has **Keep private**; **Stop keeping private** takes it back.

How others see a private game depends on who else owns it:

- **A mark is per player.** It hides that player's having the game, never another owner's.
- **Masked when every owner marked it.** Others then see "A hidden game", as for a hidden game.
- **Visible when another owner did not mark it.** If someone else who owns the game has not kept it private, the
  game shows normally. It still does not tell anyone that you have it.
- **Owners are never shown to anyone but admins.** Who owns a game is left out for every non-admin, whether or not
  anything is private.
- **Admins see everything.**
- **Marks go with the account.** When an account is removed, its private marks are removed with it.

!!! note "Why only Steam"
    Private games are about which platform account owns a game, so they apply to games a player owns through a
    linked account. Battle.net needs no private games: lanstash signs in to no Battle.net account, so nothing
    there says who owns what.
