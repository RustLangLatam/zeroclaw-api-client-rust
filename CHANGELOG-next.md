# zeroclaw-api-client — next release

Changes since v0.2.1. They become the release notes of the next version.

## What's New

None yet.

## Breaking Changes

- The client now covers only the official ZeroClaw gateway. The channel
  routes official ZeroClaw does not have are gone: `channels().send`,
  `rooms`, `create_room`, `room_members`, `invite`, `remove_member` and
  `redeliver`, with their types (`RoomSummary`, `RoomMember`, `CreatedRoom`,
  `Participant`, `Redelivered`) and the `group_chats` example. Upstream
  declined them in
  [zeroclaw-labs/zeroclaw#11434](https://github.com/zeroclaw-labs/zeroclaw/issues/11434);
  they stay on the unreleased branch `dev/fork-channel-routes`.
