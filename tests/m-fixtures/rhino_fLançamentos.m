let
    Source = Excel.Workbook(File.Contents("C:\Dados\Fluxo de Caixa Tab. Fato - Rhino.xlsx"), null, true),
    Fato_Sheet = Source{[Item="Fato",Kind="Sheet"]}[Data],
    #"Promoted Headers" = Table.PromoteHeaders(Fato_Sheet, [PromoteAllScalars=true]),
    #"Changed Type" = Table.TransformColumnTypes(#"Promoted Headers",{{"Data", type date}, {"Descrição", type text}, {"Valor", Currency.Type}, {"Categoria", type text}, {"Tipo", type text}})
in
    #"Changed Type"
