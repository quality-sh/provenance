import { defineSpec, requirement, rule, source } from "@quality-sh/provenance";

import { createShareLink } from "./share-links.js";

export const shareLinks = defineSpec("share-links")
  .requirements(
    requirement("sharing")
      .statement("Users can share the documents")
      .from(source("sharing-policy").document("docs/sharing-policy.md"))
      .rules(
        rule("expiry")
          .statement("Share links must expire in 30 days")
          .implementedBy(createShareLink),
      ),
  )
  .build();
