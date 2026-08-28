-- Example user script.
--
-- A script is a Lua module returning a table with two functions:
--
--   setup()    -- one-time requests run by every virtual user
--                 before the load phase begins (e.g. to acquire a session)
--   requests() -- the user-loop of a virtual user, cycled continuously while the
--                 load test runs when the virtual user is selected to send a request
--
-- Each function returns an array of request specs. This example defines
-- a single static spec — a table describing an HTTP GET to the `api` service.
-- The base URL for the service is provided by the service registry configuration file.

local function setup()
	-- The setup phase is optional: return an empty list to skip it.
	return {}
end

local function requests()
	return {
		{
			protocol = "http",
			method = "GET",
			service = "api",
			path = "/",
			-- headers is a table of name-value pairs to include in the request headers
			headers = {
				Accept = "application/json",
			},
			-- query is a table of key-value pairs to include in the URL query
			query = {
				greeting = "world",
			},
		},
	}
end

return { setup = setup, requests = requests }

