import { point } from "@turf/helpers";
import length from "@turf/length";
import lineSlice from "@turf/line-slice";
import lineSplit from "@turf/line-split";
import nearestPointOnLine from "@turf/nearest-point-on-line";
import type { Feature, LineString, Point, Position } from "geojson";
import type { RouteProps } from "./index";

// Splits a LineString produced from route mode by a point on (or near) the
// line. If successful, returns two new features. The properties of the input
// feature are copied, then `length_meters` and `waypoints` are fixed. The
// feature ID is not modified.
export function splitRoute(
  input: Feature<LineString, RouteProps>,
  splitPoint: Feature<Point>,
): [Feature<LineString, RouteProps>, Feature<LineString, RouteProps>] | null {
  let result = lineSplit(input, splitPoint);
  if (result.features.length != 2) {
    return null;
  }
  let piece1 = result.features[0] as Feature<LineString, RouteProps>;
  let piece2 = result.features[1] as Feature<LineString, RouteProps>;

  // lineSplit may introduce unnecessary coordinate precision
  piece1.geometry.coordinates = piece1.geometry.coordinates.map(setPrecision);
  piece2.geometry.coordinates = piece2.geometry.coordinates.map(setPrecision);

  // The properties get lost. Deep copy everything to both
  piece1.properties = JSON.parse(JSON.stringify(input.properties));
  piece2.properties = JSON.parse(JSON.stringify(input.properties));

  fixRouteProperties(input, piece1, piece2, splitPoint);

  return [piece1, piece2];
}

function fixRouteProperties(
  original: Feature<LineString, RouteProps>,
  piece1: Feature<LineString, RouteProps>,
  piece2: Feature<LineString, RouteProps>,
  splitPt: Feature<Point>,
) {
  // Fix length
  piece1.properties.length_meters =
    length(piece1, { units: "kilometers" }) * 1000.0;
  piece2.properties.length_meters =
    length(piece2, { units: "kilometers" }) * 1000.0;

  piece1.properties.waypoints = [];
  piece2.properties.waypoints = [];

  let splitDist = distanceAlongLine(original, splitPt);
  let firstPiece = true;
  // TODO Can we iterate over an array's contents and get the index at the same time?
  let i = 0;
  for (let waypt of original.properties.waypoints!) {
    let wayptDist = distanceAlongLine(original, point([waypt.lon, waypt.lat]));
    if (firstPiece) {
      if (wayptDist < splitDist) {
        piece1.properties.waypoints.push(waypt);
      } else {
        // We found where the split occurs. We'll insert a new waypoint
        // representing the split at the end of piece1 and the beginning of
        // piece2. Should that new waypoint be snapped or freehand? There are
        // 4 cases for where the split (|) happens with regards to a
        // (s)napped and (f)reehand point:
        //
        // 1) s | s
        // 2) s | f
        // 3) f | s
        // 4) f | f
        //
        // Only in case 1 should the new waypoint introduced at (|) be
        // snapped.
        // TODO Problem: in case 1, what if we split in the middle of a road,
        // far from an intersection?

        // Note i > 0; splitDist can't be before the first waypoint (distance 0)
        // TODO Edge case: somebody manages to exactly click a waypoint
        let snapped =
          waypt.snapped && original.properties.waypoints![i - 1].snapped;

        piece1.properties.waypoints.push({
          lon: splitPt.geometry.coordinates[0],
          lat: splitPt.geometry.coordinates[1],
          snapped,
        });

        firstPiece = false;
        piece2.properties.waypoints.push({
          lon: splitPt.geometry.coordinates[0],
          lat: splitPt.geometry.coordinates[1],
          snapped,
        });
        piece2.properties.waypoints.push(waypt);
      }
    } else {
      piece2.properties.waypoints.push(waypt);
    }
    i++;
  }
}

// Returns the distance of a point along a line-string from the start, in
// meters. The point should be roughly on the line.
function distanceAlongLine(line: Feature<LineString>, point: Feature<Point>) {
  // TODO Is there a cheaper way to do this?
  let start = line.geometry.coordinates[0];
  let sliced = lineSlice(start, point, line);
  return length(sliced, { units: "kilometers" }) * 1000.0;
}

// Per https://datatracker.ietf.org/doc/html/rfc7946#section-11.2, 6 decimal
// places (10cm) is plenty of precision
export function setPrecision(pt: Position): Position {
  return [Math.round(pt[0] * 10e6) / 10e6, Math.round(pt[1] * 10e6) / 10e6];
}
