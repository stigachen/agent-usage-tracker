import type { Message } from "./i18n";
import type { UsageSnapshot } from "./types";

export function grokStatus(snapshot: UsageSnapshot): Message | null {
  if (snapshot.providerId !== "grok") return null;
  switch (snapshot.credentialIssue) {
    case "missing": return "Not signed in";
    case "expired": return "Session expired";
    case "rejected": return "Sign-in rejected";
    case "unreadable": return "Can't read sign-in";
    case "invalid": return "Invalid sign-in data";
    case "unsupported": return "Unsupported sign-in";
    case "ambiguous": return "Multiple sign-ins found";
    case "changed": return "Sign-in changed";
  }
  switch (snapshot.fetchIssue) {
    case "network": return "Network request failed";
    case "service": return "Service unavailable";
    case "response": return "Invalid usage response";
  }
  if (snapshot.error) return "Couldn't load usage";
  if (snapshot.needsAuth) return "Session expired";
  return snapshot.accountId ? null : "Not signed in";
}

// Read errors must remain visible even when the file cannot supply an account ID.
// A missing login still belongs with the other unconnected providers.
export function hasGrokProblem(snapshot: UsageSnapshot): boolean {
  return snapshot.providerId === "grok" &&
    (!!snapshot.error || !!snapshot.fetchIssue || (!!snapshot.credentialIssue && snapshot.credentialIssue !== "missing"));
}
