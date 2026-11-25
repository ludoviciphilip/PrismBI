let
    Source = Query1(#date(2026, 4, 1), 275, #duration(1, 0, 0, 0)),
    #"Converted to Table" = Table.FromList(Source, Splitter.SplitByNothing(), null, null, ExtraValues.Error),
    #"Renamed Columns" = Table.RenameColumns(#"Converted to Table",{{"Column1", "Data"}}),
    #"Changed Type1" = Table.TransformColumnTypes(#"Renamed Columns",{{"Data", type date}}),
    #"Inserted Year" = Table.AddColumn(#"Changed Type1", "Year", each Date.Year([Data]), Int64.Type),
    #"Inserted Month" = Table.AddColumn(#"Inserted Year", "Month", each Date.Month([Data]), Int64.Type),
    #"Inserted Month Name" = Table.AddColumn(#"Inserted Month", "Month Name", each Date.MonthName([Data], "pt-BR"), type text),
    #"Inserted Day" = Table.AddColumn(#"Inserted Month Name", "Day", each Date.Day([Data]), Int64.Type),
    #"Inserted Day Name" = Table.AddColumn(#"Inserted Day", "Day Name", each Date.DayOfWeekName([Data], "pt-BR"), type text),
    #"Inserted Week of Month" = Table.AddColumn(#"Inserted Day Name", "Week of Month", each Date.WeekOfMonth([Data]), Int64.Type),
    #"Renamed Columns1" = Table.RenameColumns(#"Inserted Week of Month",{{"Year", "Ano"}, {"Month", "Mês"}, {"Month Name", "Nome do Mês"}, {"Day", "Dia"}, {"Day Name", "Nome do Dia"}, {"Week of Month", "Semana do Mês"}}),
    #"Added Conditional Column" = Table.AddColumn(#"Renamed Columns1", "Semestre", each if [Mês] <= 6 then "S1" else "S2"),
    #"Changed Type" = Table.TransformColumnTypes(#"Added Conditional Column",{{"Semestre", type text}}),
    #"Inserted First Characters" = Table.AddColumn(#"Changed Type", "First Characters", each Text.Start([Nome do Mês], 3), type text),
    #"Duplicated Column" = Table.DuplicateColumn(#"Inserted First Characters", "Ano", "Ano - Copy"),
    #"Merged Columns" = Table.CombineColumns(Table.TransformColumnTypes(#"Duplicated Column", {{"Ano - Copy", type text}}, "it-IT"),{"First Characters", "Ano - Copy"},Combiner.CombineTextByDelimiter("/", QuoteStyle.None),"Mês/Ano"),
    #"Capitalized Each Word" = Table.TransformColumns(#"Merged Columns",{{"Mês/Ano", Text.Proper, type text}})
in
    #"Capitalized Each Word"
