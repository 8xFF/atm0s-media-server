# Authentication and Multi Tenancy

We support authentication using JWT tokens. Additionally, we provide support for multi-tenancy using JWT tokens. The `sub` claim in the JWT token is used to identify both the user and the tenant.

When creating a session token, we include the `sub` claim in the JWT token. This claim is used to identify the tenant.

For more information on extending authentication, please refer to the Contributor Guide documentation.
