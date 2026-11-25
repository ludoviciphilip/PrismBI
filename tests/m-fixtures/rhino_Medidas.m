let
    Source = "Medidas=",
    #"Converted to Table" = #table(1, {{Source}}),
    #"Removed Bottom Rows" = Table.RemoveLastN(#"Converted to Table",1),
    #"Removed Columns" = Table.RemoveColumns(#"Removed Bottom Rows",{"Column1"})
in
    #"Removed Columns"
