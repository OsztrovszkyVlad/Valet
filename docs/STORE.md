# Microsoft Store: purchase policy

Why the Store build of Valet can keep the **Cumpără Pro** button, which opens the merchant's checkout page in the browser, and what the submission must do to stay within policy.

## Source

- Microsoft Store Policies, **document version 7.20**, `ms.date: 09/14/2026`.
  - Read on 2026-10-29 from the source of the published page: [`MicrosoftDocs/windows-dev-docs`, branch `main`, `hub/apps/publish/store-policies.md`](https://raw.githubusercontent.com/MicrosoftDocs/windows-dev-docs/main/hub/apps/publish/store-policies.md).
  - Published page: https://learn.microsoft.com/en-us/windows/apps/publish/store-policies. That host can't be reached from the build environment, so the quotes come from the repository source.
- Version 7.19 (branch `docs`, `ms.date: 09/10/2025`) has the same section 10.8. The only differences are formatting ("Xbox" vs "XBOX", bold, an updated FTC link).
- Re-read section 10.8 before each submission. The policy changes a few times a year: [change history](https://learn.microsoft.com/en-us/windows/apps/publish/store-policies-change-history).

## The rule that allows an external purchase

Policy 10.8.1, verbatim:

> The following products are required to use the Microsoft Store in-product purchase APIs for the purchase of digital goods and services. Purchase of digital goods and services includes voluntary donations that result in the user receiving digital goods or services in return for the donation, including but not limited to additional features or removal of advertising.
>
> - (a) Games (excluding games made available through a subscription in PC gaming subscription products and in-app purchases in such games)
> - (b) Products offered on XBOX consoles.
>
> […]
>
> Non-game products made available on PC devices may either use a secure third-party purchase API or the Microsoft Store in-product purchase API for in-app purchases of digital items or services that are consumed or used within the product.

Valet is a non-game product, offered only on PC, never on Xbox. So the Store purchase API is optional for it, and Pro can be sold through Paddle or Stripe Managed Payments.

The sentence in 10.8.2 about "additional features or removal of advertising" applies to voluntary donations ("to receive voluntary donations from users"). Pro is a sale, not a donation, so it doesn't apply.

## Conditions in 10.8.2, and how Valet meets them

Policy 10.8.2, verbatim: "In cases where your product's use of a secure third-party purchase API is allowed or required, the following requirements apply:"

| Requirement (verbatim) | Valet |
|---|---|
| "At the time of the transaction or when you collect any payment or financial information from the customer, your product must identify the commerce transaction provider, authenticate the user, and obtain the user's confirmation of the transaction." | The checkout page of the merchant (Paddle or Stripe) names the merchant, collects the email and payment details, and asks for confirmation. The app names the merchant under the button ("Plata se face prin Paddle, care emite factura și se ocupă de TVA.") once `crates/valet-core/license/merchant.txt` holds `Paddle` or `Stripe`. **Vlad, before submission:** commit `merchant.txt`. |
| "Your product can offer the user the ability to permanently remain authenticated, but the user must have the ability to either require an authentication on every transaction or to turn off in-product transactions." | Valet stores no payment account or login. Every purchase goes through the checkout page again. |
| "If your product collects credit card information or uses a third-party payment processor that collects credit card information, the payment processing must meet the current PCI Data Security Standard (PCI DSS)." | Card data goes only to the merchant, never through the app or the license server. Paddle and Stripe are PCI DSS level 1 processors; confirm this on the chosen merchant's own compliance page before submission. |
| "If your product requires user registration or payment transaction experience at install, it must take place in the product's in-app experience. After installation of your product is completed, users may be directed to a browser to complete registration or transactions." | No registration or payment at install. The button sends the user to the browser after installation, which the policy allows. |
| "You must note the use of a secure third-party purchase API in Partner Center during the submission process." | **Vlad, at every submission:** tick the third-party purchase option in Partner Center. |

## Other sections that apply

- **10.8.3** (verbatim): "Products from individual accounts cannot require financial information for primary functionality." Valet's primary functionality (Free and Trial) needs no payment, so an individual developer account is allowed.
- **10.8.4** (verbatim): "Your product and its associated metadata must provide information about the types of in-product purchases offered and the range of prices. You may not mislead customers and must be clear about the nature of your in-product promotions and offerings including the scope and terms of any trial experiences. If your product restricts access to user-created content during or after a trial, you must notify users in advance. In addition, your product must make it clear to users that they are initiating a purchase option in the product."
  - **Listing** (`docs/store/listing.*.md`): it names the one purchase (Pro, 29 € with VAT included, one-time, 12 months of updates), the 14-day trial and what happens after it. Rules over 3 and folders over 2 stay saved but inactive. Nothing is deleted.
  - **Advance notice in the app:** the header shows "Probă Pro: N zile" for the whole trial. In the last 3 days the trial notice also says what becomes inactive (see `PlanBadge`). The Valet Pro section explains the limits.
  - **Clear purchase:** the button reads "Cumpără Pro" and states the price next to it.
- **10.8.7** (verbatim): pricing must "Comply with all applicable laws, regulations and regulatory guidelines" and must "Not be priced irrationally high relative to the features and functionality provided by your product." 29 € sits between the small organizers and Hazel (`docs/reports/phase-2.md`, R1).
- **10.3.1** (verbatim): "If your product requires login credentials, provide us with a working demo account using the **Notes for certification** field." Valet has no login.
- **10.3.2** (verbatim): "If your product requires access to a server, the server must be functional to verify that it's working correctly." Activating Pro needs the license server, so it must be up during certification.

## Notes for certification (text for Partner Center)

Paste this into the **Notes for certification** field and adapt it:

> Valet works without an account. All features are available for 14 days from the first start (Pro trial); after that the Free plan applies (3 own rules, 2 watched folders). The "Cumpără Pro" button opens the checkout page of <merchant> in the browser (third-party purchase for a non-game PC product, policy 10.8.1). To test Pro after the trial, import the attached license file: Settings → Valet Pro → "Am o licență". The license server at https://license.valetapp.ro must be reachable. The test license is revoked after certification.

How to issue the test license: on the server, run `node issue.mjs --email store-review@valetapp.ro --days 30`, attach the file to the submission, then run `node revoke.mjs --license <id>` after certification. It uses no merchant order, so it never shows in sales.

## Not verified

- **Pricing field:** which value Partner Center expects ("Free" or "Free with in-app purchases") when the purchase happens outside the Store. Check during the first submission. The policy text only requires the third-party purchase note from 10.8.2.
- **PCI DSS status:** check the chosen merchant's current compliance page before submission.
