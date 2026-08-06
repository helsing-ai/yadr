# A fixture for yadr's end-to-end tests. Never executed; only ever read as text.

# YADR: 2024-03-02 Retry with exponential backoff
#
# In the context of talking to an upstream service that occasionally rejects requests, we
# faced the question of how quickly to retry.
#
# We decided for doubling the delay after each attempt up to a ceiling, and neglected both
# retrying immediately and giving up after the first failure.
#
# We did this to achieve recovery from brief outages without adding load to a service that is
# already struggling, accepting that a request can take much longer than usual to resolve.
#
# We think this is the right trade-off because the outages we see in practice are short, and
# a caller that cannot wait can impose its own deadline.
def fetch():
    pass
