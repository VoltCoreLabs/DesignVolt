//! Vector paths drawn with the pen: anchors with Bezier handles.

/// A point of a path with the handles of its curves. Handles are offsets from the anchor;
/// a zero handle means a straight corner on that side.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Anchor {
    pub x: f64,
    pub y: f64,
    /// The handle towards the previous anchor.
    pub in_x: f64,
    pub in_y: f64,
    /// The handle towards the next anchor.
    pub out_x: f64,
    pub out_y: f64,
}

impl Anchor {
    pub fn corner(x: f64, y: f64) -> Self {
        Self {
            x,
            y,
            ..Self::default()
        }
    }

    /// A smooth anchor: the outgoing handle is `(dx, dy)` and the incoming one mirrors it.
    pub fn smooth(x: f64, y: f64, dx: f64, dy: f64) -> Self {
        Self {
            x,
            y,
            in_x: -dx,
            in_y: -dy,
            out_x: dx,
            out_y: dy,
        }
    }
}

/// The path of a `NodeKind::Path` node. Coordinates are in the node's box as it was when the
/// path was made (`size`); when the box is resized the path is scaled with it.
#[derive(Clone, Debug, PartialEq)]
pub struct Vector {
    pub anchors: Vec<Anchor>,
    pub closed: bool,
    pub size: (f64, f64),
}

impl Vector {
    /// One line of text without tabs, for the document file: `c|o;w;h;anchor|anchor|...`
    /// with each anchor as `x,y,in_x,in_y,out_x,out_y`.
    pub fn to_code(&self) -> String {
        let anchors: Vec<String> = self
            .anchors
            .iter()
            .map(|a| {
                format!(
                    "{},{},{},{},{},{}",
                    a.x, a.y, a.in_x, a.in_y, a.out_x, a.out_y
                )
            })
            .collect();
        format!(
            "{};{};{};{}",
            if self.closed { 'c' } else { 'o' },
            self.size.0,
            self.size.1,
            anchors.join("|")
        )
    }

    pub fn from_code(s: &str) -> Option<Self> {
        let mut f = s.split(';');
        let closed = match f.next()? {
            "c" => true,
            "o" => false,
            _ => return None,
        };
        let num = |v: Option<&str>| v?.parse::<f64>().ok().filter(|v| v.is_finite());
        let (w, h) = (num(f.next())?, num(f.next())?);
        let mut anchors = Vec::new();
        for a in f.next()?.split('|') {
            let v: Vec<f64> = a
                .split(',')
                .map(|n| n.parse::<f64>().ok().filter(|v| v.is_finite()))
                .collect::<Option<_>>()?;
            let [x, y, in_x, in_y, out_x, out_y] = v[..] else {
                return None;
            };
            anchors.push(Anchor {
                x,
                y,
                in_x,
                in_y,
                out_x,
                out_y,
            });
        }
        (anchors.len() >= 2 && w > 0.0 && h > 0.0).then_some(Self {
            anchors,
            closed,
            size: (w, h),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_round_trip_and_reject_garbage() {
        let v = Vector {
            anchors: vec![
                Anchor::corner(0.0, 0.0),
                Anchor::smooth(10.5, 4.0, 3.0, -2.0),
                Anchor::corner(20.0, 0.25),
            ],
            closed: true,
            size: (20.0, 4.0),
        };
        assert_eq!(Vector::from_code(&v.to_code()), Some(v));
        assert!(Vector::from_code("x;1;1;0,0,0,0,0,0|1,1,0,0,0,0").is_none());
        assert!(
            Vector::from_code("o;1;1;0,0,0,0,0,0").is_none(),
            "one anchor"
        );
        assert!(Vector::from_code("o;1;1;0,0,0|1,1,0,0,0,0").is_none());
        assert!(Vector::from_code("o;0;1;0,0,0,0,0,0|1,1,0,0,0,0").is_none());
    }
}
