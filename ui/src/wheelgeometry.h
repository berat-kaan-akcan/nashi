#pragma once

#include <QObject>
#include <QtQmlIntegration/qqmlintegration.h>

// Pie-menu math shared by the QML wheel and the tests.
// Slice 0 is centred at 12 o'clock; slices advance clockwise.
class WheelGeometry : public QObject
{
    Q_OBJECT
    QML_ELEMENT
    QML_SINGLETON

    Q_PROPERTY(double innerRadius READ innerRadius CONSTANT)

public:
    static constexpr int Cancel = -1;  // inside the centre "cancel" disc
    static constexpr int Outside = -2; // too far away from the wheel
    static constexpr double kInnerRadius = 52.0;
    static constexpr double kOuterReach = 1.4; // hits allowed up to outer * reach
    // User size factor (Settings::wheelScale): multiplies both radii, so the
    // angles and therefore the hit test stay identical at every scale.
    static constexpr double kMinScale = 0.8;
    static constexpr double kMaxScale = 1.4;

    using QObject::QObject;

    // Index of the slice under (dx, dy) relative to the wheel centre.
    static int sliceAt(double dx, double dy, int count, double innerRadius, double outerRadius);
    // Outer radius that keeps `count` labels readable.
    static double outerRadiusFor(int count);
    static double clampScale(double scale);
    static double innerRadiusFor(double scale) { return kInnerRadius * clampScale(scale); }
    static double outerRadiusFor(int count, double scale) { return outerRadiusFor(count) * clampScale(scale); }
    // Slice start angle in degrees, Qt convention (0 = 3 o'clock, clockwise).
    static double sliceStartAngle(int index, int count);
    static double sliceSweep(int count);

    Q_INVOKABLE int hit(double dx, double dy, int count, double innerRadius, double outerRadius) const
    {
        return sliceAt(dx, dy, count, innerRadius, outerRadius);
    }
    Q_INVOKABLE double outerRadius(int count) const { return outerRadiusFor(count); }
    Q_INVOKABLE double scaledInnerRadius(double scale) const { return innerRadiusFor(scale); }
    Q_INVOKABLE double scaledOuterRadius(int count, double scale) const { return outerRadiusFor(count, scale); }
    Q_INVOKABLE double scaled(double value, double scale) const { return value * clampScale(scale); }
    Q_INVOKABLE double startAngle(int index, int count) const { return sliceStartAngle(index, count); }
    Q_INVOKABLE double sweep(int count) const { return sliceSweep(count); }
    double innerRadius() const { return kInnerRadius; }
};
