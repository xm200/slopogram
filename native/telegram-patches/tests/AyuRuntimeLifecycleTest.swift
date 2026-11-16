import Foundation
import SlopogramAyu
@main struct Check {
 static func main() async throws {
  let termination = AyuTerminationGate()
  precondition(termination.begin())
  precondition(!termination.begin())
  let path = NSTemporaryDirectory() + "ayu-ui-check-" + UUID().uuidString
  try FileManager.default.createDirectory(atPath: path, withIntermediateDirectories: true)
  defer { try? FileManager.default.removeItem(atPath: path) }
  let account = Account(path)
  AyuAccountRuntime.installGhostProvider()
  precondition(SlopogramGhostPolicy.isSuppressed(accountBasePath: path, control: .typing) == true)
  let runtime = AyuAccountRuntime.account(account)
  precondition(AyuAccountRuntime.account(Account(path)) === runtime)
  precondition(runtime.suppressAutomaticReads)
  runtime.attach(account: account); runtime.attach(account: account)
  precondition(account.stateManager.messageDeletionEvents.starts == 1)
  for _ in 0..<200 { if !runtime.suppressAutomaticReads { break }; try await Task.sleep(nanoseconds: 10_000_000) }
  precondition(!runtime.suppressAutomaticReads)
  precondition(account.postbox.versions.starts == 1)
  precondition(SlopogramGhostPolicy.isSuppressed(accountBasePath: path, control: .typing) == false)
  AyuAccountRuntime.release(account); precondition(account.stateManager.messageDeletionEvents.disposals == 0)
  AyuAccountRuntime.release(account); precondition(account.stateManager.messageDeletionEvents.disposals == 1)
  precondition(AyuAccountRuntime.account(account) === runtime)
  let enabledPath = path + "-enabled"
  defer { try? FileManager.default.removeItem(atPath: enabledPath) }
  try FileManager.default.createDirectory(atPath: enabledPath, withIntermediateDirectories: true)
  do {
   let features = AyuFeatures(dataDir: URL(fileURLWithPath: enabledPath).appendingPathComponent("slopogram-ayu").path)
   try await features.setPolicy(policy: FeaturePolicy(suppressReadReceipts: false, suppressTyping: true, suppressOnlineStatus: true, archiveObservedMessages: true))
  }
  let enabledAccount = Account(enabledPath)
  let enabledRuntime = AyuAccountRuntime.account(enabledAccount)
  precondition(enabledRuntime !== runtime)
  enabledRuntime.attach(account: enabledAccount)
  for _ in 0..<200 { if enabledAccount.postbox.versions.starts > 0 { break }; try await Task.sleep(nanoseconds: 10_000_000) }
  precondition(enabledAccount.postbox.versions.starts == 1)
  let message = AyuMessage(chatId: 7, id: 1, text: "fixture", sender: "A", outgoing: false, sentAt: 1, editedAt: nil, globalIdEligible: true, authorPeerId: nil)
  enabledRuntime.observe([message])
  await AyuAccountRuntime.flushAll()
  let copied = URL(fileURLWithPath: enabledPath).appendingPathComponent("reopened")
  try FileManager.default.createDirectory(at: copied, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
  for filename in ["observed-history.sqlite", "observed-history.sqlite-wal"] {
   let source = URL(fileURLWithPath: enabledPath).appendingPathComponent("slopogram-ayu/" + filename)
   if FileManager.default.fileExists(atPath: source.path) { try FileManager.default.copyItem(at: source, to: copied.appendingPathComponent(filename)) }
  }
  let reopened = AyuFeatures(dataDir: copied.path)
  let saved = try await reopened.history(chatId: 7)
  precondition(saved.count == 1 && saved[0].versions.contains { $0.text == "fixture" })
  let cleared = await withCheckedContinuation { continuation in enabledRuntime.clearHistoryData { continuation.resume(returning: $0) } }
  precondition(cleared)
  precondition(SlopogramGhostPolicy.isSuppressed(accountBasePath: enabledPath, control: .typing))
  precondition(SlopogramGhostPolicy.isSuppressed(accountBasePath: enabledPath, control: .online))
  precondition(enabledAccount.postbox.versions.disposals == 0)
  let emptyHistory = await withCheckedContinuation { continuation in enabledRuntime.readArchivedHistory(chat: 7) { continuation.resume(returning: $0) } }
  precondition(emptyHistory?.isEmpty == true)
  enabledRuntime.observe([message])
  let history = await withCheckedContinuation { continuation in enabledRuntime.readArchivedHistory(chat: 7) { continuation.resume(returning: $0) } }
  precondition(history?.count == 1)
  AyuAccountRuntime.release(enabledAccount)
  precondition(enabledAccount.postbox.versions.disposals == 1)
  print("Ayu lifecycle check passed")
 }
}
