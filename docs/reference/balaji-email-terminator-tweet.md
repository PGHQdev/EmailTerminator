# Origin: Balaji's "Email Terminator" product request

Source tweets by @balajis, Jul 12, 2024 (verified against screenshots):

- https://x.com/balajis/status/1811590332657598604 (109.2K views)
- https://x.com/balajis/status/1811596254918701373 (37.4K views)

## Tweet 1

> THE EMAIL TERMINATOR
>
> Wanted: an AI tool that processes your inbox, lists every SaaS subscription
> and newsletter, shows total amount billed and total number of emails sent by
> that service, and allows you to one-click cancel.

## Tweet 2 — Ideas on implementation

> 1) You can do this as both a hosted version (simple, but requires Gmail
> access) and a privacy-preserving desktop or open source version.
>
> 2) For the hosted version, look at tools like Superhuman or Emailmeter that
> actually do ingest large quantities of email and render in their own
> interface.
>
> 3) For the local version, you would instead run on mbox files downloaded
> locally (eg via Mail dot app or equivalent). That is a file format for email
> that can be parsed by many programs.
>
> 4) You would need to heavily curate the one click cancellation process as
> many services and newsletters make this difficult (or impossible) to do via
> email.
>
> 5) One idea is to at least replace every card for every service with a
> privacy.com card or equivalent, a virtual card that you can cancel on behalf
> of the user, so that you don't need to log in to every service to cancel for
> them.
>
> 6) Also recommend replacing every email for every service and newsletter
> with a privacy preserving email, similar to Apple's private relay.
>
> 7) The big red "unsubscribe all" and "cancel all" buttons should be there,
> and they should say how much inbound email will go away and how much $ you
> will save each month....
>
> 8) ...BUT you should also be careful to recommend that people not cancel
> things like AWS which can break live sites. So you can collect cancellation
> statistics and say "97% of users chose not to cancel this."
>
> 9) You should make graphs of the number of emails sent and historical price
> of each newsletter and service, to see if they went up over time. That
> itself may flag things to cancel (or alternatively to preserve, if you are
> using the service more heavily).
>
> 10) Your pricing should be as user aligned as possible. A one-time
> non-recurring charge of $99 for a privacy-preserving desktop client with an
> open source tool and great interface may actually be a good way to go.
>
> 11) You should look at previous efforts in this area like unroll.me and the
> extremely cool Google Sheets unsubscriber by @labnol. The latter is a
> brilliant application which isn't just open source but open state. It runs
> as a script in Google sheets and shows you what emails are being
> unsubscribed. You might modify that.
>
> 12) If this works, many busy people and businesses will install it. Be
> aligned with them and resist the urge to set up ads or to whitelist
> services that pay you without paying the user. Instead, if you need more
> revenue, either charge more to the user *or* allow senders to pay users (or
> a charity they desire).
>
> 13) You will likely need to combine deterministic parsing of mbox metadata,
> hard-coded heuristics, and AI-based parsing of unstructured content in
> order to do this well.
>
> 14) A great email services like @Superhuman could build this into the
> client or as a standalone dashboard at superhuman.com that's a different
> view on the data.
>
> 15) A company like 1Password could also get into this as they are all about
> credentials and trust, and might be able to implement one-click
> cancellation.
>
> 16) The general area becomes more important as inflation gets worse and
> everyone gets more aggressive with outbound email and price increases and
> so on. Many companies won't love this product, and I get it. But it is also
> defense for the user, and the customer — and that is a product in its own.
>
> 17) Eventually you want messaging and billing to be for a "throwaway"
> crypto address where which users can fill up with a balance, and where
> senders can pay to message and send encrypted messages. But that's a whole
> separate workflow.
