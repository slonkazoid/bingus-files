use crate::method::Method;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RouteToken {
    PATH(String),
    VARIABLE(String),
    WILDCARD,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Route(pub Method, pub Box<[RouteToken]>);

pub fn match_route<'a>(
    method: Method,
    path: String,
    routes: impl Iterator<Item = &'a Route>,
) -> Option<&'a Route> {
    let routes = routes.into_iter().filter(|route| route.0 == method);
    let path: Vec<&str> = path.trim_matches('/').split('/').collect();
    let mut highest_path_matches = 0;
    let mut highest_variable_matches = 0;
    let mut highest_wildcard_matches = 0;
    let mut highest_route: Option<&Route> = None;

    'route: for route in routes {
        let required_tokens = route
            .1
            .iter()
            .filter(|r| **r != RouteToken::WILDCARD)
            .count();

        if path.len() < required_tokens {
            continue;
        }

        let mut path_matches = 0;
        let mut variable_matches = 0;
        let mut wildcard_matches = 0;

        for (index, token) in route.1.iter().enumerate() {
            match token {
                RouteToken::PATH(path_token) => {
                    if path_token == path[index] {
                        path_matches += 1;
                    } else {
                        continue 'route;
                    }
                }
                RouteToken::VARIABLE(_) => {
                    variable_matches += 1;
                }
                RouteToken::WILDCARD => {
                    wildcard_matches += path.len() + 1 - index;
                }
            }
        }

        if (path.len() > path_matches + variable_matches && wildcard_matches == 0)
            || (path_matches == 0 && variable_matches == 0 && wildcard_matches == 0)
        {
            continue;
        } else if path_matches > highest_path_matches
            || (path_matches == highest_path_matches && variable_matches > highest_variable_matches)
            || (path_matches == highest_path_matches
                && variable_matches == highest_variable_matches
                && wildcard_matches < highest_wildcard_matches)
            || (path_matches == 0
                && highest_path_matches == 0
                && variable_matches == 0
                && highest_variable_matches == 0
                && wildcard_matches > highest_wildcard_matches)
        {
            highest_path_matches = path_matches;
            highest_variable_matches = variable_matches;
            highest_wildcard_matches = wildcard_matches;
            highest_route = Some(route);
        }
    }

    highest_route
}

#[cfg(test)]
mod tests {
    use crate::{
        route::{match_route, RouteToken},
        Method, Route,
    };

    macro_rules! r {
        [$($x:expr),*] => {
            Route(Method::GET, Box::new([$($x),*]))
        };
    }

    macro_rules! m {
        ($p:literal, $r:expr) => {
            match_route(Method::GET, $p.to_string(), $r.iter())
        };
    }

    #[test]
    fn sanity_check() {
        let get_slash = r![RouteToken::PATH("".to_string())];
        assert!(m!("/", [get_slash]).is_some());
    }

    #[test]
    fn route_matching() {
        let routes = [
            r![RouteToken::PATH("".to_string())],        // 0: GET /
            r![RouteToken::PATH("hello".to_string())],   // 1: GET /hello
            r![RouteToken::PATH("hi".to_string())],      // 2: GET /hi
            r![RouteToken::VARIABLE("var".to_string())], // 3: GET /:var
            r![
                RouteToken::PATH("hello".to_string()),
                RouteToken::PATH("hi".to_string())
            ], // 4: GET /hello/hi
            r![
                RouteToken::PATH("hello".to_string()),
                RouteToken::VARIABLE("var".to_string())
            ], // 5: GET /hello/:var
            r![
                RouteToken::VARIABLE("var".to_string()),
                RouteToken::PATH("hi".to_string())
            ], // 6: GET /:var/hi
            r![
                RouteToken::VARIABLE("var1".to_string()),
                RouteToken::VARIABLE("var2".to_string())
            ], // 7: GET /:var1/:var2
            r![
                RouteToken::PATH("hello".to_string()),
                RouteToken::PATH("hi".to_string()),
                RouteToken::WILDCARD
            ], // 8: GET /hello/hi/*
            r![
                RouteToken::PATH("hello".to_string()),
                RouteToken::VARIABLE("var".to_string()),
                RouteToken::WILDCARD
            ], // 9: GET /hello/:var/*
            r![RouteToken::WILDCARD],                    // 10: GET /*
        ];

        assert_eq!(m!("/", routes), Some(&routes[0]));
        assert_eq!(m!("/hello", routes), Some(&routes[1]));
        assert_eq!(m!("/hi", routes), Some(&routes[2]));
        assert_eq!(m!("/foo", routes), Some(&routes[3]));
        assert_eq!(m!("/hello/hi", routes), Some(&routes[4]));
        assert_eq!(m!("/hello/foo", routes), Some(&routes[5]));
        assert_eq!(m!("/foo/hi", routes), Some(&routes[6]));
        assert_eq!(m!("/foo/bar", routes), Some(&routes[7]));
        assert_eq!(m!("/hello/hi/foo", routes), Some(&routes[8]));
        assert_eq!(m!("/hello/foo/bar", routes), Some(&routes[9]));
        assert_eq!(m!("/foo/bar/baz", routes), Some(&routes[10]));
    }
}
