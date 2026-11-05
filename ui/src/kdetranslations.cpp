#include "kdetranslations.h"

#include <libintl.h>

namespace {

QString filled(QString text, const QVariant &a1, const QVariant &a2)
{
    if (a1.isValid())
        text.replace(QLatin1String("%1"), a1.toString());
    if (a2.isValid())
        text.replace(QLatin1String("%2"), a2.toString());
    return text;
}

} // namespace

QString KdeTranslations::i18n(const QString &text, const QVariant &a1, const QVariant &a2) const
{
    return filled(text, a1, a2);
}

QString KdeTranslations::i18nd(const QString &domain, const QString &text, const QVariant &a1, const QVariant &a2) const
{
    if (text.isEmpty())
        return {}; // gettext answers an empty text with the catalog's header
    const QByteArray name = domain.toUtf8();
    const QByteArray source = text.toUtf8();
    bind_textdomain_codeset(name.constData(), "UTF-8");
    return filled(QString::fromUtf8(dgettext(name.constData(), source.constData())), a1, a2);
}
