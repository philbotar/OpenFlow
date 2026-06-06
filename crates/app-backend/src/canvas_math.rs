pub const NODE_WIDTH: f32 = 220.0;
pub const NODE_HEIGHT: f32 = 52.0;

#[must_use]
pub fn clamp_node_position(
    pos: (f32, f32),
    node_size: (f32, f32),
    canvas_size: (f32, f32),
) -> (f32, f32) {
    let max_x = (canvas_size.0 - node_size.0).max(0.0);
    let max_y = (canvas_size.1 - node_size.1).max(0.0);
    (pos.0.clamp(0.0, max_x), pos.1.clamp(0.0, max_y))
}

#[must_use]
pub fn edge_anchor_points(
    from_pos: (f32, f32),
    to_pos: (f32, f32),
    node_size: (f32, f32),
) -> ((f32, f32), (f32, f32)) {
    (
        (
            from_pos.0 + node_size.0,
            node_size.1.mul_add(0.5, from_pos.1),
        ),
        (to_pos.0, node_size.1.mul_add(0.5, to_pos.1)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_position_inside_canvas_bounds() {
        let pos = clamp_node_position((600.0, 430.0), (200.0, 120.0), (640.0, 480.0));
        assert_eq!(pos, (440.0, 360.0));
    }

    #[test]
    fn edge_anchor_points_connect_right_to_left() {
        let (start, end) = edge_anchor_points((80.0, 120.0), (360.0, 120.0), (220.0, 120.0));
        assert_eq!(start, (300.0, 180.0));
        assert_eq!(end, (360.0, 180.0));
    }
}
