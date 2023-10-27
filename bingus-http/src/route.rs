use crate::method::Method;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RouteToken {
    PATH(Box<str>),
    PARAMETER(Box<str>),
    WILDCARD,
}

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Route(pub Method, pub Box<[RouteToken]>);

impl Route {
    pub fn new(method: Method, tokens: Box<[RouteToken]>) -> Self {
        Self(method, tokens)
    }
}

pub fn match_route<'a>(
    method: Method,
    path: Vec<&str>,
    routes: impl Iterator<Item = &'a Route>,
) -> Option<(&'a Route, usize, usize, usize)> {
    let routes = routes.into_iter().filter(|route| route.0 == method);
    let mut highest_path_matches = 0;
    let mut highest_parameter_matches = 0;
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
        let mut parameter_matches = 0;
        let mut wildcard_matches = 0;

        for (index, token) in route.1.iter().enumerate() {
            match token {
                RouteToken::PATH(path_token) => {
                    if (**path_token) == *path[index] {
                        path_matches += 1;
                    } else {
                        continue 'route;
                    }
                }
                RouteToken::PARAMETER(_) => {
                    parameter_matches += 1;
                }
                RouteToken::WILDCARD => {
                    wildcard_matches += path.len() + 1 - index;
                }
            }
        }

        if (path.len() > path_matches + parameter_matches && wildcard_matches == 0)
            || (path_matches == 0 && parameter_matches == 0 && wildcard_matches == 0)
        {
            continue;
        } else if path_matches > highest_path_matches
            || (path_matches == highest_path_matches
                && parameter_matches > highest_parameter_matches)
            || (path_matches == highest_path_matches
                && parameter_matches == highest_parameter_matches
                && wildcard_matches < highest_wildcard_matches)
            || (path_matches == 0
                && highest_path_matches == 0
                && parameter_matches == 0
                && highest_parameter_matches == 0
                && wildcard_matches > highest_wildcard_matches)
        {
            highest_path_matches = path_matches;
            highest_parameter_matches = parameter_matches;
            highest_wildcard_matches = wildcard_matches;
            highest_route = Some(route);
        }
    }

    match highest_route {
        Some(route) => Some((
            route,
            highest_path_matches,
            highest_parameter_matches,
            highest_wildcard_matches,
        )),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        route::{match_route, RouteToken},
        Method, Route,
    };

    macro_rules! route_macro {
    ($method:ident $($type:ident $($value:literal)?)/+) => {
        Route(
            Method::$method,
            Box::new([$(
                RouteToken::$type$((String::from($value).into_boxed_str()))?
            ),*])
        )
    };
}

    macro_rules! get {
    [$($type:ident $($value:literal)?)/+] => {
        route_macro!(GET $($type $($value)?)/+)
    }
}

    macro_rules! m {
        ($p:literal, $r:expr) => {
            match match_route(
                Method::GET,
                $p.trim_matches('/').split('/').collect::<Vec<&str>>(),
                $r.iter(),
            ) {
                Some(some) => Some(some.0),
                None => None,
            }
        };
    }

    #[test]
    fn sanity_check() {
        let get_slash = get![PATH ""];
        assert!(m!("/", [get_slash]).is_some());
    }

    #[test]
    fn route_matching() {
        let routes = [
            get![PATH ""],      // 0: GET /
            get![PATH "hello"], // 1: GET /hello
            get![PATH "hi"],    // 2: GET /hi
            get![PATH "var"],   // 3: GET /:var
            get![
                PATH "hello" / PATH "hi"
            ], // 4: GET /hello/hi
            get![
                PATH "hello" / PARAMETER "var"
            ], // 5: GET /hello/:var
            get![
                PARAMETER "var" / PATH "hi"
            ], // 6: GET /:var/hi
            get![
                PARAMETER "var1" / PARAMETER "var2"
            ], // 7: GET /:var1/:var2
            get![
                PATH "hello"/ PATH "hi" / WILDCARD
            ], // 8: GET /hello/hi/*
            get![
                PATH "hello"/ PARAMETER "var" / WILDCARD
            ], // 9: GET /hello/:var/*
            get![WILDCARD],     // 10: GET /*
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
