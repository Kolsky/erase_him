# erase_him
Фигнюшка, которая удаляет сообщения неприятелей из беседы вк, пока работает.
Как пользоваться:
1. Получите приватный токен с доступом к сообщениям и оффлайну (чтоб был бессрочный, необязательно) любым безопасным способом.
Например, [отсюда](https://oauth.vk.com/authorize?client_id=6121396&scope=69632&redirect_uri=https://oauth.vk.com/blank.html&display=page&response_type=token&revoke=1). Токен копируется из адресной строки.
2. Рядом со скачанной прогой создайте текстовый файл config с расширением toml. Форматы кодировки помимо UTF-8 могут не работать.
3. Добавьте строку `access_token = "ваш токен в кавычках"`.
4. Добавьте строку `id_list = [ id, страниц, через, запятую, в, числовом, формате ]`. Узнать можно [здесь](https://regvk.com/id/).
5. Сохраните файл, не меняя расширение, и запустите программу.
6. ???
7. ОНА РЯЛЬНО УДАЛЯЕТ СООБЩЕНИЯ. ПОЖАЛЕЙТЕ СВОЮ МАМУ.

Кроме того, в консоли будут выводиться номера удалённых сообщений. Сообщения можно восстановить в течение 24 часов, и потому больше нигде эти номера не хранятся.
Исходный код распространяется под текстом лицензий MIT/Apache 2.0, с использованием последней в случае неопределённости.
