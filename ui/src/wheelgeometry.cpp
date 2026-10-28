#include "wheelgeometry.h"

#include <QtMath>

int WheelGeometry::sliceAt(double dx, double dy, int count, double innerRadius, double outerRadius)
{
    const double r = std::hypot(dx, dy);
    if (r < innerRadius)
        return Cancel;
    if (count <= 0 || r > outerRadius * kOuterReach)
        return Outside;
    // Angle from 12 o'clock, clockwise, in [0, 360). Screen y grows downwards.
    double deg = qRadiansToDegrees(std::atan2(dx, -dy));
    if (deg < 0)
        deg += 360.0;
    const double sweep = 360.0 / count;
    const int idx = int(std::floor((deg + sweep / 2.0) / sweep)) % count;
    return idx;
}

double WheelGeometry::outerRadiusFor(int count)
{
    // Arc length at mid radius must leave ~84 px per label.
    const double needed = 84.0 * qMax(count, 1) / (2.0 * M_PI) + 48.0;
    return qBound(150.0, needed + kInnerRadius / 2.0, 360.0);
}

double WheelGeometry::sliceSweep(int count)
{
    return count > 0 ? 360.0 / count : 360.0;
}

double WheelGeometry::sliceStartAngle(int index, int count)
{
    const double sweep = sliceSweep(count);
    return -90.0 - sweep / 2.0 + index * sweep;
}
