# Authentication and Multi Tenancy

We support authentication by using JWT token. We also support multi tenancy by using JWT token. We use `sub` claim in JWT token to identify user and tenant.

When create a session token, we will add `sub` claim to JWT token. This claim will be used to identify tenant.

For extending authentication, please refer to Contributor Guide docs.